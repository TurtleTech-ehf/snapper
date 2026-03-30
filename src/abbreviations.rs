/// English abbreviations that should not trigger sentence breaks.
pub static EN_ABBREVIATIONS: &[&str] = &[
    // Titles
    "Mr", "Mrs", "Ms", "Dr", "Prof", "Sr", "Jr", "St", "Rev", "Gen", "Gov", "Sgt", "Cpl", "Pvt",
    "Capt", "Lt", "Col", "Maj", "Cmdr", "Adm", // Academic / scientific
    "Fig", "Figs", "Eq", "Eqs", "Ref", "Refs", "Tab", "Sec", "Ch", "Vol", "No", "Nos", "Ed", "Eds",
    "Trans", "Dept", "Thm", "Lem", "Prop", "Def", "Cor", "Rem", "Ex",
    // Latin abbreviations
    "al", "approx", "ca", "cf", "etc", "et", "ibid", "viz", // Common
    "vs", "misc", "est", "govt", "dept", "univ", "inc", "corp", "ltd", "Ave", "Blvd", "Rd", "Jan",
    "Feb", "Mar", "Apr", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec", "Mon", "Tue", "Wed",
    "Thu", "Fri", "Sat", "Sun", "pp", "pg", "pt", "pts", // Single letters (initials)
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];

/// German abbreviations.
pub static DE_ABBREVIATIONS: &[&str] = &[
    // Titles
    "Hr", "Fr", "Dr", "Prof", // Academic / publishing
    "Abb", "Bd", "Hrsg", "Kap", "Nr", "S", "Verl", "Aufl", "Jg", "Anm", "Anh", "Beil", "Tab", "Gl",
    "Abschn", "Bsp", // Address
    "Str", "Pl", // Common
    "bzw", "ca", "etc", "evtl", "ggf", "vgl", "usw", // Months
    "Jan", "Feb", "Apr", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez",
    // Single letters
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];

/// French abbreviations.
pub static FR_ABBREVIATIONS: &[&str] = &[
    // Titles
    "M", "Mme", "Mlle", "Dr", "Prof", "Me", // Academic
    "fig", "eq", "chap", "vol", "p", "pp", "ed", "trad", "n", "t", // Common
    "av", "apr", "env", "cf", "etc", // Months
    "janv", "fevr", "avr", "juil", "sept", "oct", "nov", "dec", // Single letters
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];

/// Icelandic abbreviations.
pub static IS_ABBREVIATIONS: &[&str] = &[
    // Titles
    "Hr", "Fr", "Dr", // Academic / common
    "sbr", "frk", "sk", "nr", // Months
    "jan", "feb", "mar", "apr", "jun", "jul", "aug", "sep", "okt", "nov", "des",
    // Single letters
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];

/// Polish abbreviations.
pub static PL_ABBREVIATIONS: &[&str] = &[
    // Titles
    "dr", "mgr", "prof", "doc", // Academic / publishing
    "rys", "tab", "wyd", "red", "t", "s", "nr", "poz", "zob", "por", // Common
    "ul", "al", "pl", "os", // Months
    "sty", "lut", "mar", "kwi", "maj", "cze", "lip", "sie", "wrz", "paz", "lis", "gru",
    // Single letters
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];

/// Multi-word abbreviations where the period is after the second part.
pub static EN_MULTI_ABBREVS: &[&str] = &["e.g", "i.e", "a.m", "p.m", "v.s"];
pub static DE_MULTI_ABBREVS: &[&str] = &["z.B", "d.h", "u.a", "o.g", "s.o", "u.U"];
pub static FR_MULTI_ABBREVS: &[&str] = &["c.-a-d", "p.ex"];
pub static IS_MULTI_ABBREVS: &[&str] = &["m.a", "o.fl"];
pub static PL_MULTI_ABBREVS: &[&str] = &["m.in", "t.j", "j.w", "t.zw", "b.r"];

/// Get abbreviations for a language code.
pub fn abbreviations_for_lang(lang: &str) -> &'static [&'static str] {
    match lang {
        "de" => DE_ABBREVIATIONS,
        "fr" => FR_ABBREVIATIONS,
        "is" => IS_ABBREVIATIONS,
        "pl" => PL_ABBREVIATIONS,
        _ => EN_ABBREVIATIONS,
    }
}

/// Get multi-word abbreviations for a language code.
pub fn multi_abbrevs_for_lang(lang: &str) -> &'static [&'static str] {
    match lang {
        "de" => DE_MULTI_ABBREVS,
        "fr" => FR_MULTI_ABBREVS,
        "is" => IS_MULTI_ABBREVS,
        "pl" => PL_MULTI_ABBREVS,
        _ => EN_MULTI_ABBREVS,
    }
}
