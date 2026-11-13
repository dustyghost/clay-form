#!/usr/bin/env python3
"""Generate the pinned DownsampleEdgeGraph reference; no app dependency.
Usage: make-coarse-reference.py upstream output.cpp
"""
import pathlib
import subprocess
import sys
PIN="810b7a0967c35b0dc85b4464e3835e26a756c967"
root=pathlib.Path(sys.argv[1]).resolve()
for relative in ("src/hierarchy.cpp","LICENSE.txt"):
    if subprocess.check_output(["git","-C",str(root),"show",f"{PIN}:{relative}"]) != (root/relative).read_bytes():
        raise SystemExit(f"Modified upstream {relative}")
s=(root/'src/hierarchy.cpp').read_text()
function=s[s.index('void Hierarchy::DownsampleEdgeGraph('):s.index('int Hierarchy::FixFlipSat(')]
header=r'''
#include <Eigen/Core>
#include <iostream>
#include <vector>
#include <utility>
using namespace Eigen;
inline Vector2i rshift90(Vector2i shift,int amount) {
 if (amount&1) shift=Vector2i(-shift.y(),shift.x());
 if (amount>=2) shift=-shift;
 return shift;
}
class Hierarchy {
public:
 std::vector<std::vector<Vector3i>> mFQ,mF2E;
 std::vector<std::vector<Vector2i>> mE2F,mEdgeDiff;
 std::vector<std::vector<int>> mAllowChanges,mSing,mToUpperEdges,mToUpperOrients,mToUpperFaces;
 void DownsampleEdgeGraph(std::vector<Vector3i>&,std::vector<Vector3i>&,std::vector<Vector2i>&,std::vector<int>&,int);
};
'''
main=r'''
int main() {
 int e,f;if (!(std::cin>>e>>f)) return 2;
 std::vector<Vector2i> offsets(e);std::vector<int> allow(e*2);
 std::vector<Vector3i> faces(f),rotations(f);
 for(int i=0;i<e;++i) std::cin>>offsets[i][0]>>offsets[i][1]>>allow[i*2]>>allow[i*2+1];
 for(int i=0;i<f;++i) {
  for(int k=0;k<3;++k) std::cin>>faces[i][k];
  for(int k=0;k<3;++k) std::cin>>rotations[i][k];
 }
 if (!std::cin) return 3;
 Hierarchy h;h.DownsampleEdgeGraph(rotations,faces,offsets,allow,-1);
 for(int l=0;l<h.mF2E.size();++l) {
  std::cout<<"LEVEL "<<l<<' '<<h.mEdgeDiff[l].size()<<' '<<h.mF2E[l].size()<<'\n';
  for(int i=0;i<h.mEdgeDiff[l].size();++i) std::cout<<h.mEdgeDiff[l][i][0]<<' '<<h.mEdgeDiff[l][i][1]<<' '<<h.mAllowChanges[l][i*2]<<' '<<h.mAllowChanges[l][i*2+1]<<' '<<h.mE2F[l][i][0]<<' '<<h.mE2F[l][i][1]<<'\n';
  for(int i=0;i<h.mF2E[l].size();++i) std::cout<<h.mF2E[l][i][0]<<' '<<h.mF2E[l][i][1]<<' '<<h.mF2E[l][i][2]<<' '<<h.mFQ[l][i][0]<<' '<<h.mFQ[l][i][1]<<' '<<h.mFQ[l][i][2]<<'\n';
 }
}
'''
if len(sys.argv)>3:
    if sys.argv[3]!="repair": raise SystemExit("Unknown coarse-reference variant")
    header=header.replace("#include <utility>","#include <utility>\n#include <queue>\n#include <unordered_map>")
    header=header.replace("class Hierarchy {", "class Hierarchy {")
    header=header.replace(" void DownsampleEdgeGraph", " void FixFlip();\n void PropagateEdge();\n void UpdateGraphValue(std::vector<Vector3i>&,std::vector<Vector3i>&,std::vector<Vector2i>&);\n void DownsampleEdgeGraph")
    function+=s[s.index("void Hierarchy::UpdateGraphValue("):s.index("void Hierarchy::DownsampleEdgeGraph(")]
    function+=s[s.index("void Hierarchy::FixFlip() {"):s.index("void Hierarchy::clearConstraints() {")]
    main=main[:main.index(" for(int l=0;")]+r'''
 h.FixFlip();
 std::cout<<h.mEdgeDiff[0].size()<<' '<<h.mF2E[0].size()<<'\n';
 for(int i=0;i<h.mEdgeDiff[0].size();++i) std::cout<<h.mEdgeDiff[0][i][0]<<' '<<h.mEdgeDiff[0][i][1]<<' '<<h.mAllowChanges[0][i*2]<<' '<<h.mAllowChanges[0][i*2+1]<<'\n';
 for(int i=0;i<h.mF2E[0].size();++i) std::cout<<h.mF2E[0][i][0]<<' '<<h.mF2E[0][i][1]<<' '<<h.mF2E[0][i][2]<<' '<<h.mFQ[0][i][0]<<' '<<h.mFQ[0][i][1]<<' '<<h.mFQ[0][i][2]<<'\n';
}
'''
with pathlib.Path(sys.argv[2]).open('x') as out:
    out.write(f'/* QuadriFlow {PIN}\n'+(root/'LICENSE.txt').read_text()+'\n*/\n'+header+function+main)
