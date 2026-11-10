//! A vector index held in memory, every vector compared with the query's, the nearest first. For the studio's actions
//! and a song's changes (hundreds, thousands at most) that is exact and takes microseconds; a graph over them, as a
//! database's vector index builds, pays for itself only from hundreds of thousands.

/// Vectors, each with a key.
pub struct Index<K> {
    dim: usize,
    keys: Vec<K>,
    /// each key's vector, normalized, `dim` numbers a key
    vectors: Vec<f32>,
}

impl<K> Index<K> {
    pub fn new(dim: usize) -> Index<K> {
        Index { dim, keys: Vec::new(), vectors: Vec::new() }
    }

    /// `key` with `vector`, normalized; a vector of another length, or of none, is not added
    pub fn add(&mut self, key: K, vector: &[f32]) {
        let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        if vector.len() != self.dim || norm == 0.0 {
            return;
        }
        self.keys.push(key);
        self.vectors.extend(vector.iter().map(|v| v / norm));
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The `k` keys whose vectors are nearest `query`'s by cosine, with it, the nearest first.
    pub fn nearest(&self, query: &[f32], k: usize) -> Vec<(&K, f32)> {
        let norm = query.iter().map(|v| v * v).sum::<f32>().sqrt();
        if query.len() != self.dim || norm == 0.0 {
            return Vec::new();
        }
        let mut scored: Vec<(&K, f32)> = self
            .keys
            .iter()
            .zip(self.vectors.chunks_exact(self.dim))
            .map(|(key, v)| (key, v.iter().zip(query).map(|(a, b)| a * b).sum::<f32>() / norm))
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.truncate(k);
        scored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_vectors_come_first_by_cosine_whatever_their_length() {
        let mut ix = Index::new(2);
        ix.add("right", &[2.0, 0.0]);
        ix.add("up", &[0.0, 0.5]);
        ix.add("diagonal", &[1.0, 1.0]);
        ix.add("too long", &[1.0, 0.0, 0.0]);
        ix.add("nothing", &[0.0, 0.0]);
        assert_eq!(ix.len(), 3);
        let found = ix.nearest(&[1.0, 0.1], 2);
        assert_eq!(found.iter().map(|(k, _)| **k).collect::<Vec<_>>(), ["right", "diagonal"]);
        assert!((found[0].1 - 0.995).abs() < 0.001, "{found:?}");
        assert!(ix.nearest(&[0.0, 0.0], 3).is_empty());
    }
}
