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
    /// A dwarf galaxy, or at least no globular, by name or by a spread in metallicity.
    Dwarf,
    /// A structure of the disc, not a stream of the halo.
    Disc,
    /// Wider than the census's 300 pc with no progenitor known.
    Wide,
    /// A second track of a stream the census counts under another name.
    Duplicate,
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

/// `census.csv` could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Hash, thiserror::Error)]
pub enum ParseCensusError {
    /// The census's dataset holds no `census.csv`.
    #[error("the census dataset holds no census.csv")]
    Missing,
    /// The header is not [`COLUMNS`].
    #[error("the census's header is {found}, not the expected columns")]
    Header {
        /// The header found.
        found: String,
    },
    /// A row has another number of cells than [`COLUMNS`].
    #[error("line {line} of the census has {cells} cells, not 10")]
    Cells {
        /// The line, counted from 1.
        line: usize,
        /// Its cells.
        cells: usize,
    },
    /// A cell is not one the census uses.
    #[error("line {line} of the census has `{value}` for {column}")]
    Value {
        /// The line, counted from 1.
        line: usize,
        /// The cell's column.
        column: &'static str,
        /// The cell.
        value: String,
    },
    /// `globular_like` is not `yes` exactly when `kind` is `globular`.
    #[error("line {line} of the census marks globular_like against its kind")]
    Disagrees {
        /// The line, counted from 1.
        line: usize,
    },
}

impl Census {
    /// The census in `text`, `census.csv`'s bytes.
    ///
    /// # Errors
    ///
    /// [`ParseCensusError`] if the header is not [`COLUMNS`], a row has another number of cells,
    /// a cell of `globular_like`, `kind` or `in_cut` is not one the file uses, or `globular_like`
    /// disagrees with `kind`.
    pub fn parse(text: &str) -> Result<Self, ParseCensusError> {
        let mut lines = text.lines();
        let header = lines.next().unwrap_or_default();
        if !header.split(',').eq(COLUMNS) {
            return Err(ParseCensusError::Header {
                found: header.to_owned(),
            });
        }
        let mut streams = Vec::new();
        for (i, row) in lines.enumerate() {
            let line = i + 2;
            let cells: Vec<&str> = row.split(',').collect();
            if cells.len() != COLUMNS.len() {
                return Err(ParseCensusError::Cells {
                    line,
                    cells: cells.len(),
                });
            }
            let value = |column: &'static str, value: &str| ParseCensusError::Value {
                line,
                column,
                value: value.to_owned(),
            };
            let yes = |column: &'static str, cell: &str| match cell {
                "yes" => Ok(true),
                "no" => Ok(false),
                _ => Err(value(column, cell)),
            };
            let kind = match cells[7] {
                "globular" => StreamKind::Globular,
                "dwarf" => StreamKind::Dwarf,
                "disc" => StreamKind::Disc,
                "wide" => StreamKind::Wide,
                "duplicate" => StreamKind::Duplicate,
                other => return Err(value("kind", other)),
            };
            if yes("globular_like", cells[6])? != (kind == StreamKind::Globular) {
                return Err(ParseCensusError::Disagrees { line });
            }
            streams.push(CensusStream {
                name: cells[0].to_owned(),
                kind,
                in_cut: yes("in_cut", cells[9])?,
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
             Sagittarius,Sagittarius-A20,antoja2020,28.82,6.000,3018,no,dwarf,the Sgr dwarf,yes\n\
             Svol,Svol-I21,ibata2021,7.08,1.049,130,no,duplicate,M3's stream,yes\n"
        );
        let census = Census::parse(&text).unwrap();
        assert_eq!(census.total(), 4);
        assert_eq!(census.globular_in_cut(), 1);
        assert_eq!(census.of_kind(StreamKind::Dwarf), 1);
        assert_eq!(census.of_kind(StreamKind::Duplicate), 1);
    }

    #[test]
    fn a_malformed_census_is_refused_with_its_line() {
        let text = format!("{HEADER}\nOrphan,Orphan-K23,koposov2023,17.87,0,,yes,dwarf,x,yes\n");
        assert_eq!(
            Census::parse(&text),
            Err(ParseCensusError::Disagrees { line: 2 })
        );
        assert_eq!(
            Census::parse("name,track\n"),
            Err(ParseCensusError::Header {
                found: "name,track".to_owned()
            })
        );
        assert_eq!(
            Census::parse(&format!("{HEADER}\nGD-1,GD-1-I21\n")),
            Err(ParseCensusError::Cells { line: 2, cells: 2 })
        );
        assert_eq!(
            Census::parse(&format!(
                "{HEADER}\nGD-1,a,b,1,1,1,yes,globular,thin,maybe\n"
            )),
            Err(ParseCensusError::Value {
                line: 2,
                column: "in_cut",
                value: "maybe".to_owned()
            })
        );
    }
}
