//! Static word embeddings: a vector for each piece of the vocabulary, learned from a sentence model (Model2Vec's potion
//! models, MIT), and a phrase the mean of its pieces' vectors, normalized. Nothing runs but a lookup and a sum, so a
//! phrase is embedded in microseconds on any processor, without a graphics card. The vectors are kept a byte a number,
//! each with a scale, which leaves cosines as they were to a hundredth and the model at a quarter of its size.

use crate::pieces::Pieces;

/// A model on Hugging Face as claudius was checked with it.
pub struct Source {
    /// what claudius names its own file of it (`<name>.clde`)
    pub name: &'static str,
    pub repo: &'static str,
    pub revision: &'static str,
    /// each file: its name, its size in bytes and its SHA-256
    pub files: [(&'static str, u64, &'static str); 2],
}

impl Source {
    /// where a file of it downloads from
    pub fn url(&self, file: &str) -> String {
        format!("https://huggingface.co/{}/resolve/{}/{file}", self.repo, self.revision)
    }
}

/// Model2Vec's potion-base-8M (MIT): 29,528 pieces of BERT's uncased vocabulary, 256 numbers each
pub const POTION: Source = Source {
    name: "potion-base-8M",
    repo: "minishlab/potion-base-8M",
    revision: "bf8b056651a2c21b8d2565580b8569da283cab23",
    files: [
        ("model.safetensors", 30_236_760, "f65d0f325faadc1e121c319e2faa41170d3fa07d8c89abd48ca5358d9a223de2"),
        ("vocab.txt", 219_690, "1394523a67ddd404a825428018c0582a6998bcfa044ecbcbf1f4d71adb94c61c"),
    ],
};

/// A static embedding model.
pub struct Model {
    pieces: Pieces,
    dim: usize,
    /// each piece's vector, a byte a number, `dim` of them a piece
    rows: Vec<i8>,
    /// what each piece's bytes are multiplied by
    scales: Vec<f32>,
}

/// the first bytes of the model's own file, then its version
const MAGIC: &[u8; 4] = b"CLDE";
const VERSION: u32 = 1;

impl Model {
    /// From Model2Vec's files: `model.safetensors`, whose one tensor holds a vector of 32-bit floats for each piece in
    /// the order of `vocab.txt`, one piece a line.
    pub fn from_model2vec(safetensors: &[u8], vocab: &str) -> Result<Model, String> {
        let (rows, dim, floats) = tensor(safetensors)?;
        let pieces = Pieces::new(vocab);
        if pieces.len() != rows {
            return Err(format!("the vocabulary has {} pieces and the tensor {rows} vectors", pieces.len()));
        }
        let mut quantized = Vec::with_capacity(rows * dim);
        let mut scales = Vec::with_capacity(rows);
        for row in floats.chunks_exact(dim) {
            let most = row.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            let scale = if most > 0.0 { most / 127.0 } else { 1.0 };
            scales.push(scale);
            quantized.extend(row.iter().map(|v| (v / scale).round().clamp(-127.0, 127.0) as i8));
        }
        Ok(Model { pieces, dim, rows: quantized, scales })
    }

    /// The model as its own file: `MAGIC`, the version, the dimensions, the vocabulary, then each vector's scale and its
    /// bytes, little-endian.
    pub fn to_bytes(&self) -> Vec<u8> {
        let vocab = self.vocab_text();
        let mut out = Vec::with_capacity(16 + vocab.len() + self.scales.len() * 4 + self.rows.len());
        out.extend_from_slice(MAGIC);
        for n in [VERSION, self.dim as u32, self.scales.len() as u32, vocab.len() as u32] {
            out.extend_from_slice(&n.to_le_bytes());
        }
        out.extend_from_slice(vocab.as_bytes());
        for s in &self.scales {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out.extend(self.rows.iter().map(|b| *b as u8));
        out
    }

    /// The model from its own file (`to_bytes`).
    pub fn from_bytes(bytes: &[u8]) -> Result<Model, String> {
        let word = |at: usize| -> Result<usize, String> {
            let b = bytes.get(at..at + 4).ok_or("the model file ends early")?;
            Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        };
        if bytes.get(..4) != Some(MAGIC.as_slice()) {
            return Err("not a claudius embedding model".into());
        }
        let version = word(4)?;
        if version != VERSION as usize {
            return Err(format!("an embedding model of version {version}, where this claudius reads {VERSION}"));
        }
        let (dim, rows, vocab_len) = (word(8)?, word(12)?, word(16)?);
        let mut at = 20;
        let vocab = bytes.get(at..at + vocab_len).ok_or("the model file ends in its vocabulary")?;
        let vocab = std::str::from_utf8(vocab).map_err(|_| "the model's vocabulary is not text")?;
        at += vocab_len;
        let scales: Vec<f32> = bytes
            .get(at..at + rows * 4)
            .ok_or("the model file ends in its scales")?
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        at += rows * 4;
        let data = bytes.get(at..at + rows * dim).ok_or("the model file ends in its vectors")?;
        let pieces = Pieces::new(vocab);
        if pieces.len() != rows || dim == 0 {
            return Err(format!("the model has {} pieces and {rows} vectors of {dim}", pieces.len()));
        }
        Ok(Model { pieces, dim, rows: data.iter().map(|b| *b as i8).collect(), scales })
    }

    /// how many numbers a vector has
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// `text`'s vector: the mean of its pieces' vectors, normalized. None when no word of it is in the vocabulary.
    pub fn embed(&self, text: &str) -> Option<Vec<f32>> {
        let ids = self.pieces.ids(text);
        if ids.is_empty() {
            return None;
        }
        let mut sum = vec![0.0f32; self.dim];
        for id in &ids {
            let (row, scale) = (id_row(&self.rows, *id as usize, self.dim), self.scales[*id as usize]);
            for (s, b) in sum.iter_mut().zip(row) {
                *s += f32::from(*b) * scale;
            }
        }
        let norm = sum.iter().map(|v| v * v).sum::<f32>().sqrt();
        (norm > 0.0).then(|| sum.iter().map(|v| v / norm).collect())
    }

    /// the vocabulary in its order, one piece a line
    fn vocab_text(&self) -> String {
        self.pieces.in_order().join("\n")
    }
}

fn id_row(rows: &[i8], id: usize, dim: usize) -> &[i8] {
    &rows[id * dim..(id + 1) * dim]
}

/// The one two-dimensional tensor of 32-bit floats in a safetensors file: its rows, its columns and its numbers.
fn tensor(bytes: &[u8]) -> Result<(usize, usize, Vec<f32>), String> {
    let n = bytes.get(..8).ok_or("not a safetensors file")?;
    let n = u64::from_le_bytes([n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7]]) as usize;
    let header = bytes.get(8..8 + n).ok_or("the safetensors header is cut short")?;
    let header: serde_json::Value =
        serde_json::from_slice(header).map_err(|e| format!("the safetensors header: {e}"))?;
    let (name, info) = header
        .as_object()
        .and_then(|o| o.iter().find(|(k, _)| *k != "__metadata__"))
        .ok_or("the safetensors file holds no tensor")?;
    if info["dtype"] != "F32" {
        return Err(format!("tensor '{name}' is {}, not 32-bit floats", info["dtype"]));
    }
    let shape: Vec<usize> =
        info["shape"].as_array().into_iter().flatten().filter_map(|v| v.as_u64()).map(|v| v as usize).collect();
    let [rows, dim] = shape[..] else { return Err(format!("tensor '{name}' is not two-dimensional")) };
    let offsets: Vec<usize> =
        info["data_offsets"].as_array().into_iter().flatten().filter_map(|v| v.as_u64()).map(|v| v as usize).collect();
    let [from, to] = offsets[..] else { return Err(format!("tensor '{name}' has no data offsets")) };
    let data = bytes.get(8 + n + from..8 + n + to).ok_or("the safetensors data is cut short")?;
    if data.len() != rows * dim * 4 {
        return Err(format!("tensor '{name}' holds {} bytes for {rows} by {dim} floats", data.len()));
    }
    Ok((rows, dim, data.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// a safetensors file of one tensor of 32-bit floats
    pub(crate) fn safetensors(rows: usize, dim: usize, floats: &[f32]) -> Vec<u8> {
        let header = serde_json::json!({
            "__metadata__": {"format": "pt"},
            "embeddings": {"dtype": "F32", "shape": [rows, dim], "data_offsets": [0, floats.len() * 4]}
        })
        .to_string();
        let mut out = (header.len() as u64).to_le_bytes().to_vec();
        out.extend_from_slice(header.as_bytes());
        for f in floats {
            out.extend_from_slice(&f.to_le_bytes());
        }
        out
    }

    #[test]
    fn a_phrase_is_the_mean_of_its_pieces_and_the_model_keeps_through_its_own_file() {
        let vocab = ["[UNK]", "loud", "##er", "quiet", "bass"].join("\n");
        // three dimensions: loudness, quietness, low end
        let floats = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let m = Model::from_model2vec(&safetensors(5, 3, &floats), &vocab).unwrap();
        assert_eq!(m.dim(), 3);
        let louder = m.embed("Louder!").unwrap();
        assert!((louder[0] - 1.0).abs() < 1e-3 && louder[1].abs() < 1e-6, "{louder:?}");
        let louder_bass = m.embed("louder bass").unwrap();
        let cos = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        assert!(cos(&louder_bass, &m.embed("bass").unwrap()) > 0.5);
        assert!(cos(&louder_bass, &m.embed("quiet").unwrap()).abs() < 1e-6);
        assert!(m.embed("xylophone").is_none(), "no word of it is known");
        let again = Model::from_bytes(&m.to_bytes()).unwrap();
        assert_eq!(again.embed("louder bass"), m.embed("louder bass"));
        assert!(Model::from_bytes(b"CLDE").is_err() && Model::from_bytes(b"nope").is_err());
    }

    #[test]
    fn a_vector_kept_a_byte_a_number_keeps_its_direction() {
        let vocab = ["a", "b"].join("\n");
        let floats = [0.31, -0.72, 0.05, 0.9, 0.11, 0.42];
        let m = Model::from_model2vec(&safetensors(2, 3, &floats), &vocab).unwrap();
        let v = m.embed("a").unwrap();
        let norm = (0.31f32 * 0.31 + 0.72 * 0.72 + 0.05 * 0.05).sqrt();
        for (got, want) in v.iter().zip([0.31 / norm, -0.72 / norm, 0.05 / norm]) {
            assert!((got - want).abs() < 0.01, "{v:?}");
        }
        assert!(Model::from_model2vec(&safetensors(2, 3, &floats), "a").is_err(), "a vocabulary of another size");
    }
}
