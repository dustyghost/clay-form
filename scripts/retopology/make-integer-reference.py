#!/usr/bin/env python3
"""Pinned, standalone QuadriFlow integer stage on common saved fine fields.

Usage: make-integer-reference.py upstream output.cpp
Compile with Eigen, Boost, vendored LEMON (configured config.h), and upstream
src include paths, linking vendored lemon/base.cc. No app dependency is added.
Input: audit export-field-grid. Args: max|min. Output: transplantable JSON.
Original functions are copied verbatim; no subdivision/repair is run here.
"""
from pathlib import Path
import subprocess
import sys

PIN = "810b7a0967c35b0dc85b4464e3835e26a756c967"
root = Path(sys.argv[1]).resolve()
files = ("src/parametrizer-sing.cpp", "src/parametrizer-int.cpp",
         "src/hierarchy.cpp", "src/optimizer.cpp", "src/field-math.hpp",
         "src/flow.hpp", "src/disajoint-tree.hpp", "src/config.hpp", "LICENSE.txt")
for relative in files:
    if subprocess.check_output(["git", "-C", str(root), "show", f"{PIN}:{relative}"]) != (root / relative).read_bytes():
        raise SystemExit(f"Modified upstream {relative}")

def extract(file, start, end):
    text = (root / file).read_text()
    return text[text.index(start):text.index(end)]

functions = extract("src/parametrizer-sing.cpp", "void Parametrizer::ComputeOrientationSingularities()", "void Parametrizer::AnalyzeValence()")
functions += extract("src/parametrizer-int.cpp", "void Parametrizer::BuildEdgeInfo()", "} // namespace qflow")
functions += extract("src/hierarchy.cpp", "void Hierarchy::UpdateGraphValue(", "int Hierarchy::FixFlipSat(")
functions += extract("src/optimizer.cpp", "void Optimizer::optimize_integer_constraints(", "#ifdef WITH_CUDA\n\nvoid Optimizer::optimize_orientations_cuda")
header = r'''
#include "field-math.hpp"
#include "flow.hpp"
#include "disajoint-tree.hpp"
#include <iostream>
#include <sstream>
#include <iomanip>
#include <queue>
#include <random>
#include <memory>
#include <set>
#include <stdexcept>
#include <unordered_map>
namespace qflow {
struct Hierarchy {
 std::vector<MatrixXd> mV{1},mN{1},mQ{1},mO{1},mS{1};
 MatrixXi mF;VectorXi mE2E;double mScale=1;int rng_seed=0;
 std::vector<std::vector<Vector3i>> mFQ,mF2E;
 std::vector<std::vector<Vector2i>> mE2F,mEdgeDiff;
 std::vector<std::vector<int>> mAllowChanges,mSing,mToUpperEdges,mToUpperOrients,mToUpperFaces;
 void DownsampleEdgeGraph(std::vector<Vector3i>&,std::vector<Vector3i>&,std::vector<Vector2i>&,std::vector<int>&,int);
 void UpdateGraphValue(std::vector<Vector3i>&,std::vector<Vector3i>&,std::vector<Vector2i>&);
};
struct Optimizer {
 static void optimize_integer_constraints(Hierarchy&,std::map<int,int>&,bool);
};
struct Parametrizer {
 Hierarchy hierarchy;
 std::map<int,int> singularities;
 std::map<int,Vector2i> pos_sing;
 MatrixXi pos_rank,pos_index;
 int flag_adaptive_scale=1,flag_minimum_cost_flow=0;
 std::vector<Vector2i> edge_diff;
 std::vector<DEdge> edge_values;
 std::vector<Vector3i> face_edgeIds,face_edgeOrients;
 std::vector<int> sharp_edges,allow_changes;
 std::vector<std::pair<Vector2i,int>> variables;
 void ComputeOrientationSingularities();
 void ComputePositionSingularities();
 void BuildEdgeInfo();
 void BuildIntegerConstraints();
 void ComputeMaxFlow();
};
'''
main = r'''
}
int main(int argc,char** argv){try {
 using namespace qflow;
 if(argc!=2 || (std::string(argv[1])!="min" && std::string(argv[1])!="max"))return 2;
 std::ostringstream bytes;bytes<<std::cin.rdbuf();auto raw=bytes.str();std::istringstream input(raw);
 uint64_t hash=0xcbf29ce484222325ULL;for(unsigned char c:raw)hash=(hash^c)*0x100000001b3ULL;
 int nv,nf,ne;if(!(input>>nv>>nf>>ne))return 2;
 Parametrizer p;auto& h=p.hierarchy;p.flag_minimum_cost_flow=std::string(argv[1])=="min";
 for(auto a:{&h.mV[0],&h.mN[0],&h.mQ[0],&h.mO[0]})a->resize(3,nv);
 h.mS[0].resize(2,nv);h.mF.resize(3,nf);h.mE2E=VectorXi::Constant(3*nf,-1);
 for(int v=0;v<nv;++v){for(auto a:{&h.mV[0],&h.mN[0],&h.mQ[0],&h.mO[0]})for(int k=0;k<3;++k)input>>(*a)(k,v);input>>h.mS[0](0,v);h.mS[0](1,v)=h.mS[0](0,v);}
 auto original_q=h.mQ[0];
 std::vector<Vector3i> faces(nf),rotations(nf);std::vector<int> first(ne,-1);
 for(int f=0;f<nf;++f){
  for(int k=0;k<3;++k)input>>h.mF(k,f);
  for(int k=0;k<3;++k){input>>faces[f][k];int e=faces[f][k];if(first[e]<0)first[e]=3*f+k;else{h.mE2E[first[e]]=3*f+k;h.mE2E[3*f+k]=first[e];}}
  for(int k=0;k<3;++k)input>>rotations[f][k];
 }
 std::vector<Vector2i> native(ne);std::vector<std::pair<int,int>> ends(ne);
 for(int e=0;e<ne;++e)input>>ends[e].first>>ends[e].second>>native[e].x()>>native[e].y();
 if(!input)return 3;
 auto stats=[&](const auto& offsets,const auto& turns){int closure=0,flips=0;long area=0,l1=0;for(int f=0;f<nf;++f){Vector2i d[3];for(int k=0;k<3;++k)d[k]=rshift90(offsets[faces[f][k]],turns[f][k]);closure+=(d[0]+d[1]+d[2])!=Vector2i::Zero();int a=d[0].x()*(-d[2].y())-d[0].y()*(-d[2].x());flips+=a<0;area+=std::max(0,-a);}for(int e=0;e<ne;++e)l1+=(offsets[e]-native[e]).array().abs().sum();std::cout<<"{\"closure_defects\":"<<closure<<",\"negative_faces\":"<<flips<<",\"negative_area\":"<<area<<",\"offset_l1_from_native_seed\":"<<l1<<"}";};
 std::cout<<"{\"input_fingerprint\":\""<<std::hex<<std::setfill('0')<<std::setw(16)<<hash<<std::dec<<"\",\"solver\":\""<<argv[1]<<"\",\"native_seed\":";stats(native,rotations);
 p.ComputeOrientationSingularities();p.ComputePositionSingularities();p.BuildEdgeInfo();
 int sign_edits=0;std::vector<bool> flipped(nv);for(int v=0;v<nv;++v){flipped[v]=original_q.col(v).dot(h.mQ[0].col(v))<0;sign_edits+=flipped[v];}
 if(p.edge_diff.size()!=ne || p.face_edgeIds!=faces)throw std::runtime_error("Edge IDs changed");
 for(int e=0;e<ne;++e)if(p.edge_values[e].x!=ends[e].first || p.edge_values[e].y!=ends[e].second)throw std::runtime_error("Edge endpoints changed");
 auto offsets=[&](){auto result=p.edge_diff;for(int e=0;e<ne;++e)if(flipped[ends[e].first])result[e]=-result[e];return result;};
 auto turns=[&](){auto result=p.face_edgeOrients;for(int f=0;f<nf;++f)for(int k=0;k<3;++k)if(flipped[ends[faces[f][k]].first])result[f][k]=(result[f][k]+2)%4;return result;};
 std::cout<<",\"sign_edits\":"<<sign_edits<<",\"reference_seed\":";stats(offsets(),rotations);
 p.allow_changes.assign(2*ne,1);p.sharp_edges.assign(3*nf,0);
 p.BuildIntegerConstraints();std::cout<<",\"balanced\":";stats(offsets(),turns());
 p.ComputeMaxFlow();auto final=offsets();auto final_turns=turns();
 std::cout<<",\"corrected\":";stats(final,final_turns);
 std::cout<<",\"edges\":[";for(int e=0;e<ne;++e){if(e)std::cout<<',';std::cout<<'['<<ends[e].first<<','<<ends[e].second<<','<<final[e].x()<<','<<final[e].y()<<']';}
 std::cout<<"],\"faces\":[";for(int f=0;f<nf;++f){if(f)std::cout<<',';std::cout<<'[';for(int k=0;k<3;++k)std::cout<<faces[f][k]<<',';for(int k=0;k<3;++k){if(k)std::cout<<',';std::cout<<final_turns[f][k];}std::cout<<']';}
 std::cout<<"]}\n";
 }catch(const std::exception& e){std::cerr<<e.what()<<"\n";return 1;}}
'''
with Path(sys.argv[2]).open("x") as out:
    out.write(f"/* QuadriFlow {PIN}\n" + (root / "LICENSE.txt").read_text() + "\n*/\n")
    out.write(header + functions + main)
