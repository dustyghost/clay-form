#!/usr/bin/env python3
"""Generate the complete pinned extraction/cleanup harness (offline audit only).

Compile with Eigen and upstream src include paths, linking upstream dedge.cpp.
Input: audit export-extraction. Output: positions, source memberships, quads.
"""
import pathlib
import subprocess
import sys

PIN = "810b7a0967c35b0dc85b4464e3835e26a756c967"
root = pathlib.Path(sys.argv[1]).resolve()
for relative in ("src/hierarchy.cpp", "src/parametrizer-flip.cpp",
                 "src/parametrizer-mesh.cpp", "src/dedge.cpp", "src/dedge.hpp",
                 "src/field-math.hpp", "src/disajoint-tree.hpp", "LICENSE.txt"):
    if subprocess.check_output(["git", "-C", str(root), "show", f"{PIN}:{relative}"]) != (root / relative).read_bytes():
        raise SystemExit(f"Modified upstream {relative}")
h = (root / "src/hierarchy.cpp").read_text()
f = (root / "src/parametrizer-flip.cpp").read_text()
m = (root / "src/parametrizer-mesh.cpp").read_text()
header = r'''
#include <Eigen/Dense>
#include <iostream>
#include <iomanip>
#include <vector>
#include <list>
#include <map>
#include <set>
#include <queue>
#include <unordered_map>
#include "dedge.hpp"
#include "field-math.hpp"
#include "disajoint-tree.hpp"
namespace qflow {
class Hierarchy {
public:
 std::vector<MatrixXd> mV,mO,mN,mQ;
 MatrixXi mF;
 std::vector<std::vector<Vector3i>> mFQ,mF2E;
 std::vector<std::vector<Vector2i>> mE2F,mEdgeDiff;
 std::vector<std::vector<int>> mAllowChanges,mSing,mToUpperEdges,mToUpperOrients,mToUpperFaces;
 void DownsampleEdgeGraph(std::vector<Vector3i>&,std::vector<Vector3i>&,std::vector<Vector2i>&,std::vector<int>&,int);
 void UpdateGraphValue(std::vector<Vector3i>&,std::vector<Vector3i>&,std::vector<Vector2i>&);
};
class Parametrizer {
public:
 Hierarchy hierarchy;
 DisajointTree disajoint_tree;
 std::vector<std::vector<int>> Vset;
 std::vector<Vector3d> O_compact,Q_compact,N_compact;
 std::vector<Vector4i> F_compact;
 std::set<std::pair<int,int>> Quad_edges;
 std::vector<int> V2E_compact,E2E_compact;
 VectorXi boundary_compact,nonManifold_compact;
 std::vector<double> counter;
 std::vector<int> allow_changes;
 std::vector<Vector2i> edge_diff;
 std::vector<DEdge> edge_values;
 std::vector<Vector3i> face_edgeIds,face_edgeOrients;
 void AdvancedExtractQuad();
 void BuildTriangleManifold(DisajointTree&,std::vector<int>&,std::vector<int>&,std::vector<DEdge>&,std::vector<Vector3i>&,std::vector<Vector2i>&,std::vector<Vector2i>&,std::vector<Vector3i>&);
 double QuadEnergy(std::vector<int>&,std::vector<Vector4i>&,int);
 void FixHoles();
 void FixHoles(std::vector<int>&);
 void FixValence();
};
'''
functions = h[h.index("void Hierarchy::UpdateGraphValue("):h.index("int Hierarchy::FixFlipSat(")]
functions += f[f.index("double Parametrizer::QuadEnergy("):f.index("void Parametrizer::FixFlipHierarchy()")]
functions += f[f.index("void Parametrizer::AdvancedExtractQuad()"):f.rindex("} // namespace qflow")]
functions += m[m.index("void Parametrizer::FixValence()"):m.index("void Parametrizer::OutputMesh(")]
main = r'''
}
int main(int argc, char**) {
 using namespace qflow;
 int nv,nf,ne;
 if (!(std::cin>>nv>>nf>>ne)) return 2;
 Parametrizer p;
 auto &h=p.hierarchy;
 h.mV.resize(1);h.mO.resize(1);h.mN.resize(1);h.mQ.resize(1);
 for(auto a:{&h.mV[0],&h.mO[0],&h.mN[0],&h.mQ[0]}) a->resize(3,nv);
 for(int i=0;i<nv;++i) for(auto a:{&h.mV[0],&h.mO[0],&h.mN[0],&h.mQ[0]})
  for(int k=0;k<3;++k) std::cin>>(*a)(k,i);
 h.mF.resize(3,nf);p.face_edgeIds.resize(nf);p.face_edgeOrients.resize(nf);
 for(int i=0;i<nf;++i) {
  for(int k=0;k<3;++k) std::cin>>h.mF(k,i);
  for(int k=0;k<3;++k) std::cin>>p.face_edgeIds[i][k];
  for(int k=0;k<3;++k) std::cin>>p.face_edgeOrients[i][k];
 }
 p.edge_diff.resize(ne);p.edge_values.resize(ne);p.allow_changes.resize(ne*2);
 for(int i=0;i<ne;++i) std::cin>>p.edge_values[i].x>>p.edge_values[i].y>>p.edge_diff[i][0]>>p.edge_diff[i][1]>>p.allow_changes[i*2]>>p.allow_changes[i*2+1];
 if(!std::cin) return 3;
 p.AdvancedExtractQuad();
 std::cerr<<"extracted "<<p.O_compact.size()<<' '<<p.F_compact.size()<<'\n';
 if(argc==1) p.FixValence();
 std::cerr<<"cleaned "<<p.O_compact.size()<<' '<<p.F_compact.size()<<'\n';
 std::cout<<std::setprecision(17)<<p.O_compact.size()<<' '<<p.F_compact.size()<<'\n';
 for(int i=0;i<p.O_compact.size();++i) {
  auto &v=p.O_compact[i];
  std::cout<<v.x()<<' '<<v.y()<<' '<<v.z()<<' '<<p.Vset[i].size();
  for(auto s:p.Vset[i]) std::cout<<' '<<s;
  std::cout<<'\n';
 }
 for(auto &q:p.F_compact) std::cout<<q[0]<<' '<<q[1]<<' '<<q[2]<<' '<<q[3]<<'\n';
}
'''
with pathlib.Path(sys.argv[2]).open("x") as out:
    out.write(f"/* QuadriFlow {PIN}\n" + (root / "LICENSE.txt").read_text() + "\n*/\n" + header + functions + main)
