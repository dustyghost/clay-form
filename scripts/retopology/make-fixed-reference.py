#!/usr/bin/env python3
"""Generate a comparison-only executable from pinned QuadriFlow source.

Usage: make-fixed-reference.py /path/to/QuadriFlow output.cpp
Compile separately with Eigen and QuadriFlow's src directory on the include path.
This copies the upstream function verbatim, except removing its class qualifier.
No C++ dependency is introduced into the Rust crate or the application.
"""
import pathlib
import subprocess
import sys

PIN = "810b7a0967c35b0dc85b4464e3835e26a756c967"
root = pathlib.Path(sys.argv[1]).resolve()
out = pathlib.Path(sys.argv[2])
head = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
if head != PIN:
    raise SystemExit(f"Expected QuadriFlow {PIN}, got {head}")
for relative in ("src/optimizer.cpp", "src/field-math.hpp", "src/disajoint-tree.hpp", "LICENSE.txt"):
    committed = subprocess.check_output(["git", "-C", str(root), "show", f"{PIN}:{relative}"])
    if committed != (root / relative).read_bytes():
        raise SystemExit(f"Modified reference source: {relative}")
source = (root / "src/optimizer.cpp").read_text()
function = source[source.index("void Optimizer::optimize_positions_fixed("):source.index("void Optimizer::optimize_integer_constraints(")]
function = function.replace("void Optimizer::optimize_positions_fixed(", "void optimize_positions_fixed(", 1)
math = (root / "src/field-math.hpp").read_text()
rotation = math[math.index("inline Vector3d rotate90_by("):math.index("inline Vector2i rshift90(")]
matching = math[math.index("inline std::pair<int, int> compat_orientation_extrinsic_index_4("):math.index("inline std::pair<Vector3d, Vector3d> compat_orientation_extrinsic_4(")]
license_text = (root / "LICENSE.txt").read_text()
header = r'''
#include <Eigen/Core>
#include <Eigen/Geometry>
#include <Eigen/Sparse>
#include <iostream>
#include <iomanip>
#include <map>
#include <set>
#include <unordered_map>
#include <vector>
#include "disajoint-tree.hpp"
namespace qflow {
using namespace Eigen;
template<class T> using LinearSolver = Eigen::SimplicialLLT<T>;
struct DEdge { int x, y; };
struct Hierarchy {
    std::vector<MatrixXd> mV{1}, mN{1}, mQ{1}, mO{1}, mS{1};
    MatrixXi mF;
    double mScale = 1;
};
'''
main = r'''
}
int main() {
    using namespace qflow;
    int n, e;
    if (!(std::cin >> n >> e) || n <= 0 || e <= 0) return 2;
    Hierarchy h;
    h.mF.resize(3, 0);
    for (auto p : {&h.mV[0], &h.mN[0], &h.mQ[0], &h.mO[0]}) p->resize(3, n);
    h.mS[0].resize(2, n);
    for (int i = 0; i < n; ++i) {
        for (auto p : {&h.mV[0], &h.mN[0], &h.mQ[0], &h.mO[0]})
            for (int j = 0; j < 3; ++j) std::cin >> (*p)(j, i);
        std::cin >> h.mS[0](0, i);
        h.mS[0](1, i) = h.mS[0](0, i);
    }
    std::vector<DEdge> edges(e);
    std::vector<Vector2i> offsets(e);
    for (int i = 0; i < e; ++i) std::cin >> edges[i].x >> edges[i].y >> offsets[i][0] >> offsets[i][1];
    if (!std::cin) return 3;
    std::set<int> sharp;
    std::map<int, std::pair<Vector3d, Vector3d>> constraints;
    optimize_positions_fixed(h, edges, offsets, sharp, constraints, 1);
    std::cout << std::setprecision(17);
    for (int i = 0; i < n; ++i) std::cout << h.mO[0](0,i) << ' ' << h.mO[0](1,i) << ' ' << h.mO[0](2,i) << '\n';
}
'''
with out.open("x") as stream:
    stream.write(f"/* QuadriFlow {PIN}\n{license_text}\n*/\n" + header + rotation + matching + function + main)
