#!/usr/bin/env python3
"""Pinned QuadriFlow field optimizer on identical exported fine-level input.

Usage: make-field-reference.py upstream output.cpp [--upstream-adjacency]
Compile with upstream src, upstream 3rd/pcg32, and Eigen include paths. Run the
executable with export-field input and a new output filename. It changes only
directions/origins: fine geometry, normals, areas, spacing and adjacency are
supplied by Rust. Uniform sizing, no constraints, no native runtime dependency.
The optional experiment replaces only fine neighbour order with the pinned
directed-edge fan traversal; neighbour membership and geometry must not change.
"""
from pathlib import Path
import subprocess
import sys

PIN = '810b7a0967c35b0dc85b4464e3835e26a756c967'
root = Path(sys.argv[1]).resolve()
fan_order = len(sys.argv) == 4 and sys.argv[3] == '--upstream-adjacency'
if len(sys.argv) not in (3, 4) or (len(sys.argv) == 4 and not fan_order):
    raise SystemExit(__doc__)
for relative in ('src/hierarchy.cpp', 'src/hierarchy.hpp', 'src/optimizer.cpp',
                 'src/optimizer.hpp', 'src/field-math.hpp', 'src/config.hpp',
                 'src/serialize.hpp', 'src/adjacent-matrix.hpp',
                 'src/adjacent-matrix.cpp', 'src/dedge.hpp', 'src/dedge.cpp',
                 '3rd/pcg32/pcg32/pcg32.h', 'LICENSE.txt'):
    if subprocess.check_output(['git', '-C', str(root), 'show', f'{PIN}:{relative}']) != (root / relative).read_bytes():
        raise SystemExit(f'Modified upstream {relative}')
h = (root / 'src/hierarchy.cpp').read_text()
o = (root / 'src/optimizer.cpp').read_text()
functions = h[h.index('Hierarchy::Hierarchy()'):h.index('void Hierarchy::SaveToFile(')]
functions += o[o.index('void Optimizer::optimize_orientations('):o.index('void Optimizer::optimize_scale(')]
functions += o[o.index('void Optimizer::optimize_positions('):o.index('void Optimizer::optimize_positions_dynamic(')]
header = r'''
#include <iostream>
#include <iomanip>
#include <fstream>
#include <set>
#include <queue>
#include <unordered_map>
#include "hierarchy.hpp"
#include "optimizer.hpp"
#include "pcg32/pcg32.h"
namespace qflow {
'''
main = r'''
}
int main(int argc,char** argv){try {
 using namespace qflow;
 if(argc!=3)throw std::runtime_error("usage: field-reference input output");
 std::ifstream in(argv[1]);
 std::string magic,identity;int nv,nf;double scale;
 if(!(in>>magic>>identity>>nv>>nf>>scale) || magic!="CLAYFIELD1" || nv<4 || nv>150000 || nf<4 || nf>300000 || !std::isfinite(scale) || scale<=0)
  throw std::runtime_error("Invalid field input header");
 Hierarchy h;
 h.mV[0].resize(3,nv);h.mN[0].resize(3,nv);h.mA[0].resize(nv);h.mF.resize(3,nf);
 for(int v=0;v<nv;++v){
  for(auto matrix:{&h.mV[0],&h.mN[0]})for(int k=0;k<3;++k)in>>(*matrix)(k,v);
  in>>h.mA[0][v];
  if(!h.mV[0].col(v).allFinite() || !h.mN[0].col(v).allFinite() || std::abs(h.mN[0].col(v).norm()-1)>1e-6 || !std::isfinite(h.mA[0][v]) || h.mA[0][v]<=0)
   throw std::runtime_error("Invalid fine geometry");
 }
 for(int f=0;f<nf;++f)for(int k=0;k<3;++k){in>>h.mF(k,f);if(h.mF(k,f)<0 || h.mF(k,f)>=nv)throw std::runtime_error("Invalid triangle");}
 h.mAdj[0].resize(nv);
 for(int v=0;v<nv;++v){int count;in>>count;if(count<1 || count>=nv)throw std::runtime_error("Invalid adjacency");
  for(int k=0;k<count;++k){int other;in>>other;if(other<0 || other>=nv || other==v)throw std::runtime_error("Invalid neighbour");h.mAdj[0][v].push_back(Link(other));}}
 if(!in)throw std::runtime_error("Truncated field input");
 std::string extra;if(in>>extra)throw std::runtime_error("Trailing field input");
 auto original_v=h.mV[0],original_n=h.mN[0];auto original_a=h.mA[0];auto original_f=h.mF;
 h.rng_seed=0;h.Initialize(scale,0);
 Optimizer::optimize_orientations(h);Optimizer::optimize_positions(h,0);
 if(h.mV[0]!=original_v || h.mN[0]!=original_n || h.mA[0]!=original_a || h.mF!=original_f)throw std::runtime_error("Reference modified shared geometry");
 std::ifstream existing(argv[2]);if(existing.good())throw std::runtime_error("Output exists");
 std::ofstream out(argv[2]);out<<"CLAYFIELD1 "<<identity<<' '<<nv<<'\n'<<std::setprecision(17);
 for(int v=0;v<nv;++v){for(auto matrix:{&h.mQ[0],&h.mO[0]})for(int k=0;k<3;++k)out<<(*matrix)(k,v)<<' ';out<<'\n';}
 if(!out)throw std::runtime_error("Failed field output");
 std::cerr<<"levels="<<h.mV.size()<<" vertices="<<nv<<" faces="<<nf<<" seed=0 uniform unconstrained; shared geometry unchanged\n";
 }catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
'''
if fan_order:
    d = (root / 'src/dedge.cpp').read_text()
    a = (root / 'src/adjacent-matrix.cpp').read_text()
    functions += d[d.index('inline int dedge_prev('):d.index('void compute_direct_graph_quad(')]
    functions += a[a.index('void generate_adjacency_matrix_uniform('):a.index('} // namespace qflow')]
    header = header.replace('#include <set>', '#include <set>\n#include <atomic>\n#include "dedge.hpp"')
    main = main.replace(' h.rng_seed=0;', r'''
 VectorXi v2e,e2e,boundary,nonmanifold;
 if(!compute_direct_graph(h.mV[0],h.mF,v2e,e2e,boundary,nonmanifold) || nonmanifold.sum()!=0)
  throw std::runtime_error("Invalid manifold fan input");
 AdjacentMatrix fan;
 generate_adjacency_matrix_uniform(h.mF,v2e,e2e,nonmanifold,fan);
 int changed=0;
 for(int v=0;v<nv;++v){
  std::vector<int> x,y;for(auto l:h.mAdj[0][v])x.push_back(l.id);for(auto l:fan[v])y.push_back(l.id);
  changed+=x!=y;std::sort(x.begin(),x.end());std::sort(y.begin(),y.end());
  if(x!=y)throw std::runtime_error("Fan changed neighbour membership");
 }
 h.mAdj[0]=std::move(fan);
 std::cerr<<"upstream fan order; changed rows="<<changed<<"; identical neighbour membership\n";
 h.rng_seed=0;''')
with Path(sys.argv[2]).open('x') as out:
    out.write(f'/* QuadriFlow {PIN}\n' + (root / 'LICENSE.txt').read_text() + '\n*/\n')
    out.write(header + functions + main)
