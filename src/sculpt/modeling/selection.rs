//! Topological selection across opposite edges of visible quad faces.
use super::*;

impl Mesh {
    /// Parallel edges across a connected quad strip, rather than every edge
    /// that happens to point in the same world-space direction.
    pub fn select_edge_ring(&self, seed: [usize; 2]) -> Result<Vec<[usize; 2]>, String> {
        self.quad_selection_strip(seed).map(|(edges, _)| edges)
    }

    /// Choose one of a quad's two loop directions by its entry/exit edge.
    pub fn select_face_loop(&self, face: usize, through: [usize; 2]) -> Result<Vec<usize>, String> {
        let polygons = self.polygons();
        let p = polygons.get(face).ok_or("Choose a visible quad face")?;
        if !p.visible
            || p.vertices.len() != 4
            || !p
                .edges()
                .any(|(a, b)| edge(a, b) == edge(through[0], through[1]))
        {
            return Err("Face loops need a visible quad and one of its edges".into());
        }
        self.quad_selection_strip(through).map(|(_, faces)| faces)
    }

    #[allow(clippy::type_complexity)]
    fn quad_selection_strip(
        &self,
        seed: [usize; 2],
    ) -> Result<(Vec<[usize; 2]>, Vec<usize>), String> {
        self.validate()?;
        let polygons = self.polygons();
        let mut incidence = BTreeMap::<_, Vec<_>>::new();
        for (i, p) in polygons.iter().enumerate() {
            for (a, b) in p.edges() {
                incidence.entry(edge(a, b)).or_default().push(i);
            }
        }
        let seed = edge(seed[0], seed[1]);
        if !incidence
            .get(&seed)
            .is_some_and(|fs| fs.iter().any(|&f| polygons[f].visible))
        {
            return Err("Choose a visible polygon edge".into());
        }
        let mut edges = BTreeSet::from([seed]);
        let mut faces = BTreeSet::new();
        let mut pending = vec![seed];
        while let Some(current) = pending.pop() {
            let neighbors = &incidence[&current];
            // Do not choose an arbitrary branch on non-manifold topology.
            if neighbors.len() > 2 {
                continue;
            }
            for &f in neighbors {
                let p = &polygons[f];
                if !p.visible || p.vertices.len() != 4 || !faces.insert(f) {
                    continue;
                }
                let k = p.edges().position(|(a, b)| edge(a, b) == current).unwrap();
                let opposite = edge(p.vertices[(k + 2) % 4], p.vertices[(k + 3) % 4]);
                if edges.insert(opposite) {
                    pending.push(opposite);
                }
            }
        }
        Ok((edges.into_iter().collect(), faces.into_iter().collect()))
    }
}
