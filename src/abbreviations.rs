/// Common abbreviations that should not trigger sentence breaks.
/// These are periods that do NOT end a sentence.
pub static ABBREVIATIONS: &[&str] = &[
    // Titles
    "Mr", "Mrs", "Ms", "Dr", "Prof", "Sr", "Jr", "St", "Rev", "Gen", "Gov", "Sgt", "Cpl", "Pvt",
    "Capt", "Lt", "Col", "Maj", "Cmdr", "Adm", // Academic / scientific
    "Fig", "Figs", "Eq", "Eqs", "Ref", "Refs", "Tab", "Sec", "Ch", "Vol", "No", "Nos", "Ed", "Eds",
    "Trans", "Dept", "Thm", "Lem", "Prop", "Def", "Cor", "Rem", "Ex",
    // Latin abbreviations
    "al", "approx", "ca", "cf", "etc", "et", "ibid", "viz", // Common abbreviations
    "vs", "misc", "est", "govt", "dept", "univ", "inc", "corp", "ltd", "Ave", "Blvd", "Rd", "Jan",
    "Feb", "Mar", "Apr", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec", "Mon", "Tue", "Wed",
    "Thu", "Fri", "Sat", "Sun", "pp", "pg", "pt", "pts", // Single letters (initials)
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];

/// Multi-word abbreviations where the period is after the second part.
/// These are patterns like "e.g." and "i.e." that the Unicode segmenter
/// may split at the internal period.
pub static MULTI_ABBREVS: &[&str] = &["e.g", "i.e", "a.m", "p.m", "v.s"];
