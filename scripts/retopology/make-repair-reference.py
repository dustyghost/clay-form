#!/usr/bin/env python3
"""Extract pinned QuadriFlow's complete hierarchy SAT repair for offline comparison.

Usage: make-repair-reference.py upstream output.cpp
Compile with Eigen; run: harness solver output-directory < coarse-input.txt
The algorithm and its axis-lock indexing are unchanged. Only executable discovery
and the unsafe shell/file SAT runner are replaced. Each SAT call keeps the original
8 second budget. Use an external process/memory budget as well. JSONL events and
one full graph checkpoint per radius are written to the new output directory.
This GPL reference is an audit tool, never an application dependency.
"""
from pathlib import Path
import subprocess
import sys

PIN = "810b7a0967c35b0dc85b4464e3835e26a756c967"
root = Path(sys.argv[1]).resolve()
for relative in ("src/hierarchy.cpp", "src/localsat.cpp", "LICENSE.txt"):
    expected = subprocess.check_output(["git", "-C", str(root), "show", f"{PIN}:{relative}"])
    if expected != (root / relative).read_bytes():
        raise SystemExit(f"Modified upstream {relative}")
h = (root / "src/hierarchy.cpp").read_text()
s = (root / "src/localsat.cpp").read_text()
functions = h[h.index("void Hierarchy::UpdateGraphValue("):h.index("void Hierarchy::FixFlip() {")]
begin = functions.index('    if (system("which minisat')
end = functions.index("    auto& F2E = mF2E[depth];", begin)
functions = functions[:begin] + functions[end:]
sat = s[s.index("SolverStatus SolveSatProblem("):s.index("void ExportLocalSat(")]
header = r'''
#include <Eigen/Core>
#include <algorithm>
#include <cassert>
#include <chrono>
#include <cstdio>
#include <deque>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <map>
#include <queue>
#include <stdexcept>
#include <sys/wait.h>
#include <unistd.h>
#include <fcntl.h>
#include <vector>
using namespace Eigen;
#define lprintf(...) fprintf(stderr, __VA_ARGS__)
enum class SolverStatus { Sat, Unsat, Timeout };
std::string solver_path, output_directory;
int call_number=0;
std::ofstream events;
SolverStatus RunCNF(const std::string&, int n, int seconds,
 const std::vector<std::vector<int>>& clauses, std::vector<int>& values) {
 auto stem=output_directory+"/call-"+std::to_string(call_number++);
 auto input=stem+".cnf", output=stem+".solution";
 std::ofstream out(input);
 out<<"p cnf "<<3*n<<" "<<clauses.size()<<"\n";
 for(const auto& c:clauses){for(int x:c)out<<x<<" ";out<<"0\n";}
 out.close();if(!out)throw std::runtime_error("Cannot write CNF");
 auto started=std::chrono::steady_clock::now();
 pid_t pid=fork();if(pid<0)throw std::runtime_error("fork failed");
 if(pid==0){
  int log=open((stem+".log").c_str(),O_WRONLY|O_CREAT|O_EXCL,0600);
  if(log<0)_exit(127);dup2(log,1);dup2(log,2);close(log);
  auto budget=std::to_string(seconds);
  execl("/usr/bin/timeout","timeout","--kill-after=1",budget.c_str(),
    solver_path.c_str(),input.c_str(),output.c_str(),(char*)nullptr);_exit(127);
 }
 int status=0;while(waitpid(pid,&status,0)<0){if(errno!=EINTR)throw std::runtime_error("waitpid failed");}
 int code=WIFEXITED(status)?WEXITSTATUS(status):128+WTERMSIG(status);
 double elapsed=std::chrono::duration<double>(std::chrono::steady_clock::now()-started).count();
 const char* result=code==10?"sat":code==20?"unsat":"unknown";
 events<<"{\"event\":\"solve\",\"call\":"<<call_number-1<<",\"variables\":"<<3*n
  <<",\"clauses\":"<<clauses.size()<<",\"exit\":"<<code<<",\"result\":\""<<result
  <<"\",\"seconds\":"<<elapsed<<"}"<<std::endl;
 if(code==20)return SolverStatus::Unsat;
 if(code!=10)return SolverStatus::Timeout;
 std::ifstream in(output);std::string word;in>>word;
 if(word!="SAT")throw std::runtime_error("Invalid SAT output");
 std::vector<int> assignment(3*n+1,0);int literal;
 while(in>>literal && literal!=0){
  if(abs(literal)>3*n||assignment[abs(literal)]!=0)throw std::runtime_error("Invalid model literal");
  assignment[abs(literal)]=literal>0?1:-1;
 }
 for(const auto& c:clauses){bool good=false;for(int x:c)good|=assignment[abs(x)]==(x>0?1:-1);
  if(!good)throw std::runtime_error("SAT model fails original clause");}
 auto next=values;
 for(int i=0;i<n;++i){int count=0;for(int v=-1;v<=1;++v){int a=assignment[3*i+v+2];
  if(a==0)throw std::runtime_error("Incomplete model");
  if((a>0)==(values[i]!=v)){next[i]=v;++count;}}
  if(count!=1)throw std::runtime_error("Ambiguous model");}
 values=std::move(next);return SolverStatus::Sat;
}
SolverStatus SolveSatProblem(int,std::vector<int>&,const std::vector<bool>,
 const std::vector<Vector3i>&,const std::vector<Vector3i>&,
 const std::vector<Vector4i>&,const std::vector<Vector2i>&,int=8);
Vector2i rshift90(Vector2i v,int r){if(r&1)v=Vector2i(-v.y(),v.x());if(r>=2)v=-v;return v;}
class Hierarchy {
public:
 std::vector<std::vector<Vector3i>> mFQ,mF2E;
 std::vector<std::vector<Vector2i>> mE2F,mEdgeDiff;
 std::vector<std::vector<int>> mAllowChanges,mSing,mToUpperEdges,mToUpperOrients,mToUpperFaces;
 void DownsampleEdgeGraph(std::vector<Vector3i>&,std::vector<Vector3i>&,std::vector<Vector2i>&,std::vector<int>&,int);
 void UpdateGraphValue(std::vector<Vector3i>&,std::vector<Vector3i>&,std::vector<Vector2i>&);
 int FixFlipSat(int,int);
 void PushDownwardFlip(int);
};
'''
main = r'''
int main(int argc,char** argv){try{
 if(argc!=3)return 2;solver_path=std::filesystem::absolute(argv[1]);output_directory=argv[2];
 if(!std::filesystem::create_directory(output_directory))throw std::runtime_error("Output already exists");
 events.open(output_directory+"/events.jsonl");
 int ne,nf;std::cin>>ne>>nf;if(ne<=0||ne>1000000||nf<=0||nf>1000000)return 2;
 std::vector<Vector2i> offsets(ne);std::vector<int> allow(ne*2);
 std::vector<Vector3i> faces(nf),rotations(nf);
 for(int i=0;i<ne;++i)std::cin>>offsets[i][0]>>offsets[i][1]>>allow[i*2]>>allow[i*2+1];
 for(int i=0;i<nf;++i){for(int k=0;k<3;++k)std::cin>>faces[i][k];for(int k=0;k<3;++k)std::cin>>rotations[i][k];}
 if(!std::cin)return 3;
 const auto original=offsets;const auto original_faces=faces;const auto original_rotations=rotations;
 auto area=[&](int f){auto a=rshift90(offsets[faces[f][0]],rotations[f][0]);auto b=rshift90(offsets[faces[f][1]],rotations[f][1]);return a.x()*b.y()-a.y()*b.x();};
 std::vector<int> original_areas(nf);for(int f=0;f<nf;++f)original_areas[f]=area(f);
 auto checkpoint=[&](int radius){
  int flips=0,closure=0,locks=0,changed=0,new_flips=0,negative_area=0;
  for(int f=0;f<nf;++f){int a=area(f);flips+=a<0;negative_area+=std::max(0,-a);new_flips+=a<0&&original_areas[f]>=0;
   Vector2i sum=Vector2i::Zero();for(int k=0;k<3;++k)sum+=rshift90(offsets[faces[f][k]],rotations[f][k]);closure+=sum!=Vector2i::Zero();}
  for(int e=0;e<ne;++e){changed+=offsets[e]!=original[e];for(int a=0;a<2;++a)locks+=!allow[2*e+a]&&offsets[e][a]!=original[e][a];}
  if(faces!=original_faces||rotations!=original_rotations)throw std::runtime_error("Reference changed input frames");
  events<<"{\"event\":\"checkpoint\",\"radius\":"<<radius<<",\"flips\":"<<flips
   <<",\"negative_area\":"<<negative_area<<",\"closure\":"<<closure<<",\"changed_locks\":"<<locks
   <<",\"changed_edges\":"<<changed<<",\"new_inversions\":"<<new_flips<<"}"<<std::endl;
  std::ofstream out(output_directory+"/radius-"+std::to_string(radius)+".txt");out<<ne<<" "<<nf<<"\n";
  for(int e=0;e<ne;++e)out<<offsets[e].x()<<" "<<offsets[e].y()<<" "<<allow[2*e]<<" "<<allow[2*e+1]<<"\n";
  for(int f=0;f<nf;++f)out<<faces[f][0]<<" "<<faces[f][1]<<" "<<faces[f][2]<<" "<<rotations[f][0]<<" "<<rotations[f][1]<<" "<<rotations[f][2]<<"\n";
  if(!out)throw std::runtime_error("Cannot write checkpoint");
 };
 checkpoint(0);
 // The same schedule as Parametrizer::FixFlipSat, with observation only.
 for(int threshold=1;threshold<=4;++threshold){
  Hierarchy h;h.DownsampleEdgeGraph(rotations,faces,offsets,allow,-1);int nflip=0;
  for(int depth=std::min(5,(int)h.mFQ.size()-1);depth>=0;--depth){
   events<<"{\"event\":\"stage\",\"radius\":"<<threshold<<",\"depth\":"<<depth<<"}"<<std::endl;
   nflip=h.FixFlipSat(depth,threshold);if(depth>0)h.PushDownwardFlip(depth);if(nflip==0)break;
  }
  h.UpdateGraphValue(rotations,faces,offsets);checkpoint(threshold);if(nflip==0)break;
 }
 return 0;
 }catch(const std::exception& e){std::cerr<<e.what()<<"\n";return 1;}}
'''
with Path(sys.argv[2]).open("x") as out:
    out.write(f"/* QuadriFlow {PIN}\n" + (root / "LICENSE.txt").read_text() + "\n*/\n")
    out.write(header + sat + functions + main)
