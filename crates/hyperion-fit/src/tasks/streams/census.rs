//! The census of known streams the fit counts against (P15.T11.b): `data/stream_census/`'s
//! `census.csv`, one row per stream of galstreams v1.2.1 (Mateu 2023, MNRAS 520, 5225), each
//! marked globular-like or not and inside the observer's cut or not by its `reduce.py`.

/// The census file's columns, in order.
pub const COLUMNS: [&str; 10] = [
    "name",
    "track",
    "ref",
    "distance_mid_kpc",
    "width_deg",
    "width_pc",
    "globular_like",
    "kind",
    "reason",
    "in_cut",
];

/// What a stream's progenitor was, as the census marks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StreamKind {
    /// A globular cluster, named or by the stream's thinness.
    Globular,
    /// A dwarf galaxy, by name or by a spread in metallicity.
    Dwarf,
    /// A structure of the disc, not a stream of the halo.
    Disc,
    /// Wider than the census's 300 pc with no progenitor known.
    Wide,
}

/// One stream of the census.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CensusStream {
    /// Its galstreams name.
    pub name: String,
    /// What its progenitor was.
    pub kind: StreamKind,
    /// Whether some point of its track lies within the cut's distance of the Sun and beyond its
    /// latitude (`reduce.py`).
    pub in_cut: bool,
}

/// The census, read from `census.csv`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Census {
    /// Every stream, in the file's order.
    pub streams: Vec<CensusStream>,
}

impl Census {
    /// The census in `text`, `census.csv`'s bytes.
    ///
    /// # Errors
    ///
    /// A message naming the line, if the header is not [`COLUMNS`], a row has another number of
    /// cells, or a cell of `globular_like`, `kind` or `in_cut` is not one the file uses, or if
    /// `globular_like` disagrees with `kind`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let header: Vec<&str> = lines.next().unwrap_or_default().split(',').collect();
        if header != COLUMNS {
            return Err(format!("line 1: the header is {header:?}, not {COLUMNS:?}"));
        }
        let mut streams = Vec::new();
        for (i, line) in lines.enumerate() {
            let at = i + 2;
            let cells: Vec<&str> = line.split(',').collect();
            if cells.len() != COLUMNS.len() {
                return Err(format!(
                    "line {at}: {} cells, not {}",
                    cells.len(),
                    COLUMNS.len()
                ));
            }
            let yes = |cell: &str, column: &str| match cell {
                "yes" => Ok(true),
                "no" => Ok(false),
                _ => Err(format!("line {at}: {column} is `{cell}`, not yes or no")),
            };
            let kind = match cells[7] {
                "globular" => StreamKind::Globular,
                "dwarf" => StreamKind::Dwarf,
                "disc" => StreamKind::Disc,
                "wide" => StreamKind::Wide,
                other => return Err(format!("line {at}: kind `{other}` is unknown")),
            };
            if yes(cells[6], "globular_like")? != (kind == StreamKind::Globular) {
                return Err(format!(
                    "line {at}: globular_like `{}` disagrees with kind `{}`",
                    cells[6], cells[7]
                ));
            }
            streams.push(CensusStream {
                name: cells[0].to_owned(),
                kind,
                in_cut: yes(cells[9], "in_cut")?,
            });
        }
        Ok(Self { streams })
    }

    /// The globular-like streams inside the cut: the count the fit matches.
    #[must_use]
    pub fn globular_in_cut(&self) -> u32 {
        let n = self
            .streams
            .iter()
            .filter(|s| s.kind == StreamKind::Globular && s.in_cut)
            .count();
        // A census of some hundreds of streams: the saturation is never reached.
        u32::try_from(n).unwrap_or(u32::MAX)
    }

    /// The streams of `kind`.
    #[must_use]
    pub fn of_kind(&self, kind: StreamKind) -> u32 {
        let n = self.streams.iter().filter(|s| s.kind == kind).count();
        // As in `globular_in_cut`.
        u32::try_from(n).unwrap_or(u32::MAX)
    }

    /// Every stream of the census.
    #[must_use]
    pub fn total(&self) -> u32 {
        // As in `globular_in_cut`.
        u32::try_from(self.streams.len()).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str =
        "name,track,ref,distance_mid_kpc,width_deg,width_pc,globular_like,kind,reason,in_cut";

    #[test]
    fn the_census_counts_its_globular_like_streams_inside_the_cut() {
        let text = format!(
            "{HEADER}\nGD-1,GD-1-I21,ibata2021,7.62,0.652,87,yes,globular,thin,yes\n\
             Pal15,Pal15-M17,myeong2017,38.40,0.100,67,yes,globular,progenitor Palomar 15,no\n\
             Sagittarius,Sagittarius-A20,antoja2020,28.82,6.000,3018,no,dwarf,the Sgr dwarf,yes\n"
        );
        let census = Census::parse(&text).unwrap();
        assert_eq!(census.streams.len(), 3);
        assert_eq!(census.globular_in_cut(), 1);
        assert_eq!(census.of_kind(StreamKind::Dwarf), 1);
    }

    #[test]
    fn a_row_whose_mark_disagrees_with_its_kind_is_refused() {
        let text = format!("{HEADER}\nOrphan,Orphan-K23,koposov2023,17.87,0,,yes,dwarf,x,yes\n");
        assert_eq!(
            Census::parse(&text),
            Err("line 2: globular_like `yes` disagrees with kind `dwarf`".to_owned())
        );
        assert!(Census::parse("name,track\n").is_err());
        let short = format!("{HEADER}\nGD-1,GD-1-I21\n");
        assert_eq!(
            Census::parse(&short),
            Err("line 2: 2 cells, not 10".to_owned())
        );
    }
}
