#!/usr/bin/env python3
"""Compare native pairwise seeds with pinned QuadriFlow triangle-wise seeds.

Usage: make-initial-grid-reference.py upstream output.cpp
Compile with Eigen and upstream src include paths. Input is audit
export-initial-grid. Output is a JSON comparison, not a repaired lattice.
No field optimization or flow is run. Upstream's orientation sign edits are
converted back to the input vertex frames before comparing integer offsets.
"""
from pathlib import Path
import subprocess
import sys

PIN = "810b7a0967c35b0dc85b4464e3835e26a756c967"
root = Path(sys.argv[1]).resolve()
for relative in ("src/parametrizer-sing.cpp", "src/parametrizer-int.cpp", "src/field-math.hpp", "LICENSE.txt"):
    if subprocess.check_output(["git", "-C", str(root), "show", f"{PIN}:{relative}"]) != (root / relative).read_bytes():
        raise SystemExit(f"Modified upstream {relative}")
s = (root / "src/parametrizer-sing.cpp").read_text()
i = (root / "src/parametrizer-int.cpp").read_text()
functions = s[s.index("void Parametrizer::ComputeOrientationSingularities()"):s.index("void Parametrizer::AnalyzeValence()")]
functions += i[i.index("void Parametrizer::BuildEdgeInfo()"):i.index("void Parametrizer::BuildIntegerConstraints()")]
header = r'''
#include "field-math.hpp"
#include <iostream>
#include <map>
#include <stdexcept>
#include <unordered_map>
namespace qflow {
struct Hierarchy {
 std::vector<MatrixXd> mV{1},mN{1},mQ{1},mO{1},mS{1};
 MatrixXi mF;VectorXi mE2E;double mScale=1;
};
struct Parametrizer {
 Hierarchy hierarchy;
 std::map<int,int> singularities;
 std::map<int,Vector2i> pos_sing;
 MatrixXi pos_rank,pos_index;
 int flag_adaptive_scale=1;
 std::vector<Vector2i> edge_diff;
 std::vector<DEdge> edge_values;
 std::vector<Vector3i> face_edgeIds;
 void ComputeOrientationSingularities();
 void ComputePositionSingularities();
 void BuildEdgeInfo();
};
'''
main = r'''
}
int main(){try {
 using namespace qflow;
 int nv,nf,ne;if(!(std::cin>>nv>>nf>>ne))return 2;
 Parametrizer p;auto& h=p.hierarchy;
 for(auto a:{&h.mV[0],&h.mN[0],&h.mQ[0],&h.mO[0]})a->resize(3,nv);
 h.mS[0].resize(2,nv);h.mF.resize(3,nf);h.mE2E=VectorXi::Constant(3*nf,-1);
 for(int v=0;v<nv;++v){for(auto a:{&h.mV[0],&h.mN[0],&h.mQ[0],&h.mO[0]})for(int k=0;k<3;++k)std::cin>>(*a)(k,v);std::cin>>h.mS[0](0,v);h.mS[0](1,v)=h.mS[0](0,v);}
 auto original_q=h.mQ[0];
 std::vector<Vector3i> faces(nf),rotations(nf);std::vector<int> first(ne,-1);
 for(int f=0;f<nf;++f){
  for(int k=0;k<3;++k)std::cin>>h.mF(k,f);
  for(int k=0;k<3;++k){std::cin>>faces[f][k];int e=faces[f][k];if(first[e]<0)first[e]=3*f+k;else{h.mE2E[first[e]]=3*f+k;h.mE2E[3*f+k]=first[e];}}
  for(int k=0;k<3;++k)std::cin>>rotations[f][k];
 }
 std::vector<Vector2i> native(ne);std::vector<std::pair<int,int>> ends(ne);
 for(int e=0;e<ne;++e)std::cin>>ends[e].first>>ends[e].second>>native[e].x()>>native[e].y();
 if(!std::cin)return 3;
 p.ComputeOrientationSingularities();p.ComputePositionSingularities();p.BuildEdgeInfo();
 std::map<std::pair<int,int>,Vector2i> upstream;
 int sign_edits=0;
 for(int v=0;v<nv;++v)sign_edits+=original_q.col(v).dot(h.mQ[0].col(v))<0;
 for(int e=0;e<p.edge_diff.size();++e){auto v=p.edge_values[e];auto d=p.edge_diff[e];if(original_q.col(v.x).dot(h.mQ[0].col(v.x))<0)d=-d;upstream[{v.x,v.y}]=d;}
 std::vector<Vector2i> reference(ne);int changed=0;long delta_l1=0;
 for(int e=0;e<ne;++e){reference[e]=upstream.at(ends[e]);changed+=reference[e]!=native[e];delta_l1+=(reference[e]-native[e]).array().abs().sum();}
 auto stats=[&](const std::vector<Vector2i>& offsets){int closure=0,flips=0;long area=0;for(int f=0;f<nf;++f){Vector2i d[3];for(int k=0;k<3;++k)d[k]=rshift90(offsets[faces[f][k]],rotations[f][k]);closure+=(d[0]+d[1]+d[2])!=Vector2i::Zero();int a=d[0].x()*d[1].y()-d[0].y()*d[1].x();flips+=a<0;area+=std::max(0,-a);}std::cout<<"{\"closure_defects\":"<<closure<<",\"negative_faces\":"<<flips<<",\"negative_area\":"<<area<<"}";};
 std::cout<<"{\"audit_only\":true,\"stage\":\"initial_offsets_before_flow\",\"vertices\":"<<nv<<",\"faces\":"<<nf<<",\"edges\":"<<ne<<",\"upstream_sign_edits\":"<<sign_edits<<",\"changed_edges\":"<<changed<<",\"offset_l1_difference\":"<<delta_l1<<",\"native\":";stats(native);std::cout<<",\"reference\":";stats(reference);std::cout<<"}\n";
 }catch(const std::exception& e){std::cerr<<e.what()<<"\n";return 1;}}
'''
with Path(sys.argv[2]).open("x") as out:
    out.write(f"/* QuadriFlow {PIN}\n" + (root / "LICENSE.txt").read_text() + "\n*/\n")
    out.write(header + functions + main)
