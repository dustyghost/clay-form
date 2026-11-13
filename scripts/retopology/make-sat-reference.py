"""Extract pinned upstream scalar CNF; substitute only its process runner.
Usage: make-sat-reference.py upstream output.cpp
Input to the compiled harness: edge/face counts; offset x/y and allowed x/y per
edge; three edge IDs and three rotations per face. DIMACS goes to stdout."""
from pathlib import Path
import subprocess
import sys
root=Path(sys.argv[1]).resolve(); pin='810b7a0967c35b0dc85b4464e3835e26a756c967'
for file in ['src/localsat.cpp','LICENSE.txt']:
 assert subprocess.check_output(['git','-C',str(root),'show',f'{pin}:{file}'])==(root/file).read_bytes()
s=(root/'src/localsat.cpp').read_text();body=s[s.index('SolverStatus SolveSatProblem('):s.index('void ExportLocalSat(')]
header=r'''
#include <Eigen/Core>
#include <iostream>
#include <vector>
#include <cassert>
#include <cstdio>
using namespace Eigen;
#define lprintf(...) fprintf(stderr, __VA_ARGS__)
enum class SolverStatus { Sat, Unsat, Timeout };
SolverStatus RunCNF(const std::string &, int n, int, const std::vector<std::vector<int>>& clauses, std::vector<int>&) {
 std::cout<<"p cnf "<<3*n<<" "<<clauses.size()<<"\n";
 for(auto &c:clauses){for(int x:c)std::cout<<x<<" ";std::cout<<"0\n";}
 return SolverStatus::Timeout;
}
Vector2i rot(Vector2i v,int r) {for(int i=0;i<(r%4+4)%4;++i)v=Vector2i(-v[1],v[0]);return v;}
'''
main=r'''
int main() {
 int ne,nf;std::cin>>ne>>nf;
 std::vector<int> values(2*ne);std::vector<bool> flexible(2*ne);
 for(int i=0;i<ne;++i){int x,y,a,b;std::cin>>x>>y>>a>>b;values[2*i]=x;values[2*i+1]=y;flexible[2*i]=a;flexible[2*i+1]=b;}
 std::vector<Vector3i> ve,ce;std::vector<Vector4i> vg;std::vector<Vector2i> cg;
 for(int f=0;f<nf;++f){Vector3i edges,r;for(int k=0;k<3;++k)std::cin>>edges[k];for(int k=0;k<3;++k)std::cin>>r[k];
  Vector2i vars[3],cst[3];for(int k=0;k<3;++k){auto t=rot(Vector2i(2*edges[k]+1,2*edges[k]+2),r[k]);for(int a=0;a<2;++a){cst[k][a]=t[a]>0?1:-1;vars[k][a]=abs(t[a])-1;}}
  for(int a=0;a<2;++a){ve.push_back(Vector3i(vars[0][a],vars[1][a],vars[2][a]));ce.push_back(Vector3i(cst[0][a],cst[1][a],cst[2][a]));}
  vg.push_back(Vector4i(vars[0][0],vars[1][1],vars[0][1],vars[1][0]));cg.push_back(Vector2i(cst[0][0]*cst[1][1],cst[0][1]*cst[1][0]));
 }
 if(!std::cin)return 2;
 SolveSatProblem(2*ne,values,flexible,ve,ce,vg,cg,8);
}
'''
with Path(sys.argv[2]).open('x') as f:f.write('/* '+pin+'\n'+(root/'LICENSE.txt').read_text()+'*/\n'+header+body+main)
