import { a0 as attr_class, a1 as attr, e as escape_html, $ as derived, a2 as stringify, a3 as ensure_array_like, a4 as clsx, a5 as attr_style } from "../../chunks/index.js";
function html(value) {
  var html2 = String(value ?? "");
  var open = "<!---->";
  return open + html2 + "<!---->";
}
function HeroSection($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      logoSrc,
      logoAlt = "Logo",
      title,
      accentWord,
      tagline,
      primaryCta,
      secondaryCta,
      class: className = ""
    } = $$props;
    function splitTitle(t, accent) {
      if (!accent) return { before: t, accent: "", after: "" };
      const idx = t.indexOf(accent);
      if (idx === -1) return { before: t, accent: "", after: "" };
      return {
        before: t.slice(0, idx),
        accent: t.slice(idx, idx + accent.length),
        after: t.slice(idx + accent.length)
      };
    }
    const parts = derived(() => splitTitle(title, accentWord));
    $$renderer2.push(`<section${attr_class(`hero ${stringify(className)}`, "svelte-lgkl54")}><div class="hero-inner svelte-lgkl54"><img class="hero-logo svelte-lgkl54"${attr("src", logoSrc)}${attr("alt", logoAlt)}/> <h1 class="svelte-lgkl54">${escape_html(parts().before)}`);
    if (parts().accent) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<span class="accent svelte-lgkl54">${escape_html(parts().accent)}</span>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]-->${escape_html(parts().after)}</h1> <p class="hero-tagline svelte-lgkl54">${escape_html(tagline)}</p> `);
    if (primaryCta || secondaryCta) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="hero-ctas svelte-lgkl54">`);
      if (primaryCta) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<a class="hero-cta primary svelte-lgkl54"${attr("href", primaryCta.href)}>`);
        if (primaryCta.icon) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<span class="cta-icon svelte-lgkl54">${html(primaryCta.icon)}</span>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--> ${escape_html(primaryCta.text)}</a>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--> `);
      if (secondaryCta) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<a class="hero-cta secondary svelte-lgkl54"${attr("href", secondaryCta.href)}>`);
        if (secondaryCta.icon) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<span class="cta-icon svelte-lgkl54">${html(secondaryCta.icon)}</span>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--> ${escape_html(secondaryCta.text)}</a>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div> <div class="scroll-hint svelte-lgkl54" aria-hidden="true"><svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="svelte-lgkl54"><path d="M12 5v14M5 12l7 7 7-7" class="svelte-lgkl54"></path></svg></div></section>`);
  });
}
function SectionHeader($$renderer, $$props) {
  let { label, title, subtitle, class: className = "" } = $$props;
  $$renderer.push(`<div${attr_class(`section-header ${stringify(className)}`, "svelte-1qukhag")}>`);
  if (label) {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<span class="section-label svelte-1qukhag">${escape_html(label)}</span>`);
  } else {
    $$renderer.push("<!--[-1-->");
  }
  $$renderer.push(`<!--]--> <h2 class="svelte-1qukhag">${escape_html(title)}</h2> `);
  if (subtitle) {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<p class="section-subtitle svelte-1qukhag">${escape_html(subtitle)}</p>`);
  } else {
    $$renderer.push("<!--[-1-->");
  }
  $$renderer.push(`<!--]--></div>`);
}
function FeatureGrid($$renderer, $$props) {
  let { features, class: className = "" } = $$props;
  $$renderer.push(`<div${attr_class(`feature-grid ${stringify(className)}`, "svelte-dsn5cs")}><!--[-->`);
  const each_array = ensure_array_like(features);
  for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
    let feature = each_array[$$index];
    $$renderer.push(`<div class="feature-card svelte-dsn5cs"><div class="feature-icon svelte-dsn5cs">${html(feature.icon)}</div> <h3 class="svelte-dsn5cs">${escape_html(feature.title)}</h3> <p class="svelte-dsn5cs">${escape_html(feature.description)}</p></div>`);
  }
  $$renderer.push(`<!--]--></div>`);
}
function InstallCards($$renderer, $$props) {
  let { options, class: className = "" } = $$props;
  $$renderer.push(`<div${attr_class(`install-grid ${stringify(className)}`, "svelte-1vau96a")}><!--[-->`);
  const each_array = ensure_array_like(options);
  for (let $$index_1 = 0, $$length = each_array.length; $$index_1 < $$length; $$index_1++) {
    let option = each_array[$$index_1];
    $$renderer.push(`<div class="install-card svelte-1vau96a"><div class="install-header svelte-1vau96a">${escape_html(option.title)}</div> <pre class="install-code svelte-1vau96a"><!--[-->`);
    const each_array_1 = ensure_array_like(option.lines);
    for (let $$index = 0, $$length2 = each_array_1.length; $$index < $$length2; $$index++) {
      let line = each_array_1[$$index];
      $$renderer.push(`<span${attr_class(clsx(line.type || "plain"), "svelte-1vau96a")}>${escape_html(line.text)}</span>
`);
    }
    $$renderer.push(`<!--]--></pre></div>`);
  }
  $$renderer.push(`<!--]--></div>`);
}
function ProductFooter($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      license = "MIT License",
      licenseUrl,
      repoUrl,
      links = [],
      class: className = ""
    } = $$props;
    $$renderer2.push(`<footer${attr_class(`product-footer ${stringify(className)}`, "svelte-1dua93w")}><div class="container svelte-1dua93w"><p class="footer-meta svelte-1dua93w">`);
    if (licenseUrl) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<a${attr("href", licenseUrl)} class="svelte-1dua93w">${escape_html(license)}</a>`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`${escape_html(license)}`);
    }
    $$renderer2.push(`<!--]--> `);
    if (repoUrl) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<span class="sep svelte-1dua93w">·</span> <a${attr("href", repoUrl)} class="svelte-1dua93w">GitHub</a>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></p> <div class="footer-brand svelte-1dua93w"><span>provided by</span> <a href="https://turtletech.us/" class="brand-link svelte-1dua93w"><svg class="turtle-logo svelte-1dua93w" viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg"><g stroke="#595959" stroke-width="2" stroke-linecap="round" fill="none"><path d="M22 44 C18 48, 12 50, 10 52" fill="#80D25B"></path><path d="M22 44 C18 48, 12 50, 10 52" transform="matrix(-1 0 0 1 64 0)" fill="#80D25B"></path><path d="M26 20 C24 16, 20 14, 18 12" fill="#80D25B"></path><path d="M26 20 C24 16, 20 14, 18 12" transform="matrix(-1 0 0 1 64 0)" fill="#80D25B"></path><ellipse cx="32" cy="34" rx="14" ry="12" fill="#BD7575"></ellipse><polygon points="32,24 26,30 26,38 32,44 38,38 38,30" fill="none" stroke="#595959" stroke-width="1.5"></polygon><line x1="26" y1="30" x2="38" y2="30"></line><line x1="26" y1="38" x2="38" y2="38"></line><line x1="32" y1="24" x2="32" y2="22"></line><circle cx="28" cy="14" r="1.5" fill="#595959"></circle><circle cx="24" cy="14" r="1.5" fill="#595959"></circle><path d="M22 16 C24 18, 28 18, 30 16" fill="#80D25B"></path></g></svg> TurtleTech ehf.</a></div> `);
    if (links.length > 0) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<nav class="footer-links svelte-1dua93w"><!--[-->`);
      const each_array = ensure_array_like(links);
      for (let i = 0, $$length = each_array.length; i < $$length; i++) {
        let link = each_array[i];
        if (i > 0) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<span class="sep svelte-1dua93w">·</span>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--> <a${attr("href", link.href)} class="svelte-1dua93w">${escape_html(link.text)}</a>`);
      }
      $$renderer2.push(`<!--]--></nav>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div></footer>`);
  });
}
function DarkModeToggle($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { class: className = "" } = $$props;
    $$renderer2.push(`<button${attr_class(`theme-toggle ${stringify(className)}`, "svelte-1gfxg8s")}${attr("aria-label", "Switch to dark mode")}${attr("title", "Dark mode")}>`);
    {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"></path></svg>`);
    }
    $$renderer2.push(`<!--]--></button>`);
  });
}
function LangSelector($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      languages,
      class: className = ""
    } = $$props;
    let activeLang = languages[0]?.code || "en";
    $$renderer2.push(`<div${attr_class(`lang-selector ${stringify(className)}`, "svelte-1vzo3nc")} role="group" aria-label="Language"><!--[-->`);
    const each_array = ensure_array_like(languages);
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let lang = each_array[$$index];
      $$renderer2.push(`<button${attr_class("lang-btn svelte-1vzo3nc", void 0, { "active": activeLang === lang.code })}>${escape_html(lang.label)}</button>`);
    }
    $$renderer2.push(`<!--]--></div>`);
  });
}
function NoiseTexture($$renderer, $$props) {
  let { opacity = 0.025, class: className = "" } = $$props;
  $$renderer.push(`<div${attr_class(`noise-overlay ${stringify(className)}`, "svelte-16w0ur4")}${attr_style(`opacity: ${stringify(opacity)};`)} aria-hidden="true"><svg width="100%" height="100%" class="svelte-16w0ur4"><filter id="ttech-noise"><feTurbulence type="fractalNoise" baseFrequency="0.85" numOctaves="4" stitchTiles="stitch"></feTurbulence></filter><rect width="100%" height="100%" filter="url(#ttech-noise)"></rect></svg></div>`);
}
function ScrollReveal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      threshold = 0.1,
      rootMargin = "0px 0px -40px 0px",
      class: className = "",
      children
    } = $$props;
    let visible = false;
    $$renderer2.push(`<div${attr_class(`reveal ${stringify(className)}`, "svelte-1fcoqme", { "visible": visible })}>`);
    children($$renderer2);
    $$renderer2.push(`<!----></div>`);
  });
}
function DemoSection($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { t } = $$props;
    let activeTab = "split";
    $$renderer2.push(`<div class="demo-wrapper"><div class="demo-tabs svelte-h9lsrp"><button${attr_class("demo-tab first svelte-h9lsrp", void 0, { "active": activeTab === "split" })}>${escape_html(t.tab_before_after)}</button> <button${attr_class("demo-tab svelte-h9lsrp", void 0, { "active": activeTab === "diff" })}>${escape_html(t.tab_diff)}</button> <button${attr_class("demo-tab last svelte-h9lsrp", void 0, { "active": activeTab === "sdiff" })}>${escape_html(t.tab_sdiff)}</button></div> <div${attr_class("demo-panel svelte-h9lsrp", void 0, { "active": activeTab === "split" })}><div class="split-view svelte-h9lsrp"><div class="split-pane pane-before svelte-h9lsrp"><span class="split-pane-label svelte-h9lsrp">${escape_html(t.pane_before)}</span> We present a method for formatting prose documents with semantic line breaks, where each sentence occupies its own line. This approach dramatically reduces noise in version control diffs when multiple authors collaborate on a manuscript. Our tool handles LaTeX, Org-mode, Markdown, and plaintext with format-aware parsing that preserves structural elements.</div> <div class="split-pane pane-after svelte-h9lsrp"><span class="split-pane-label svelte-h9lsrp">${escape_html(t.pane_after)}</span> <span class="sentence svelte-h9lsrp">We present a method for formatting prose documents with semantic line breaks, where each sentence occupies its own line.</span> <span class="sentence svelte-h9lsrp">This approach dramatically reduces noise in version control diffs when multiple authors collaborate on a manuscript.</span> <span class="sentence svelte-h9lsrp">Our tool handles LaTeX, Org-mode, Markdown, and plaintext with format-aware parsing that preserves structural elements.</span></div></div></div> <div${attr_class("demo-panel svelte-h9lsrp", void 0, { "active": activeTab === "diff" })}><div class="diff-view svelte-h9lsrp"><div class="diff-scenario svelte-h9lsrp"><span class="scenario-label svelte-h9lsrp">Without snapper: change one word, diff shows entire paragraph</span> <span class="diff-file svelte-h9lsrp">--- a/paper.org</span> <span class="diff-file svelte-h9lsrp">+++ b/paper.org</span> <span class="diff-hunk svelte-h9lsrp">@@ -1,3 +1,3 @@</span> <span class="diff-rm svelte-h9lsrp">-We present a method for formatting prose documents with semantic line breaks, where each sentence occupies its own line. This approach dramatically reduces noise in version control diffs when multiple authors collaborate on a manuscript. Our tool handles LaTeX, Org-mode, Markdown, and plaintext with format-aware parsing that preserves structural elements.</span> <span class="diff-add svelte-h9lsrp">+We present a method for formatting prose documents with semantic line breaks, where each sentence occupies its own line. This approach <span class="diff-highlight svelte-h9lsrp">significantly</span> reduces noise in version control diffs when multiple authors collaborate on a manuscript. Our tool handles LaTeX, Org-mode, Markdown, and plaintext with format-aware parsing that preserves structural elements.</span></div> <div class="diff-scenario svelte-h9lsrp"><span class="scenario-label svelte-h9lsrp">With snapper: same change, diff shows one line</span> <span class="diff-file svelte-h9lsrp">--- a/paper.org</span> <span class="diff-file svelte-h9lsrp">+++ b/paper.org</span> <span class="diff-hunk svelte-h9lsrp">@@ -1,3 +1,3 @@</span> <span class="diff-ctx svelte-h9lsrp">We present a method for formatting prose documents with semantic line breaks, where each sentence occupies its own line.</span> <span class="diff-rm svelte-h9lsrp">-This approach dramatically reduces noise in version control diffs when multiple authors collaborate on a manuscript.</span> <span class="diff-add svelte-h9lsrp">+This approach <span class="diff-highlight svelte-h9lsrp">significantly</span> reduces noise in version control diffs when multiple authors collaborate on a manuscript.</span> <span class="diff-ctx svelte-h9lsrp">Our tool handles LaTeX, Org-mode, Markdown, and plaintext with format-aware parsing that preserves structural elements.</span></div></div></div> <div${attr_class("demo-panel svelte-h9lsrp", void 0, { "active": activeTab === "sdiff" })}><div class="diff-view svelte-h9lsrp"><div class="diff-header svelte-h9lsrp">${escape_html(t.sdiff_header)}</div> <div class="diff-scenario svelte-h9lsrp"><span class="scenario-label svelte-h9lsrp">${escape_html(t.sdiff_scenario)}</span> <span class="diff-file svelte-h9lsrp">$ snapper sdiff paper_v1.org paper_v2.org</span> <span class="diff-hunk svelte-h9lsrp">@@ -1,3 +1,3 @@</span> <span class="diff-ctx svelte-h9lsrp">We present a method for formatting prose documents with semantic line breaks, where each sentence occupies its own line.</span> <span class="diff-rm svelte-h9lsrp">-This approach dramatically reduces noise in version control diffs when multiple authors collaborate on a manuscript.</span> <span class="diff-add svelte-h9lsrp">+This approach <span class="diff-highlight svelte-h9lsrp">significantly</span> reduces noise in version control diffs when multiple authors collaborate on a manuscript.</span> <span class="diff-ctx svelte-h9lsrp">Our tool handles LaTeX, Org-mode, Markdown, and plaintext with format-aware parsing that preserves structural elements.</span></div> <div class="diff-scenario svelte-h9lsrp"><span class="scenario-label svelte-h9lsrp">${escape_html(t.sdiff_reflow)}</span> <span class="diff-file svelte-h9lsrp">$ snapper sdiff wrapped_at_80.org semantic_breaks.org</span> <span class="diff-ctx svelte-h9lsrp" style="color: #6a6; font-style: italic;">No sentence-level differences.</span></div></div></div></div>`);
  });
}
const I18N = {
  en: {
    hero_title: "Snap your prose into clean, <accent>diff-friendly</accent> lines",
    hero_tagline: "A fast, format-aware semantic line break formatter for academic papers and collaborative writing.",
    cta_github: "View on GitHub",
    cta_docs: "Documentation",
    demo_label: "See it in action",
    demo_title: "One sentence, one line",
    demo_subtitle: "Traditional wrapping hides changes in paragraph-wide diffs. Snapper breaks at sentence boundaries so every edit shows exactly what changed.",
    tab_before_after: "Before & After",
    tab_diff: "Git Diff",
    tab_sdiff: "Sentence Diff",
    pane_before: "Before (traditional wrapping)",
    pane_after: "After (snapper)",
    sdiff_header: "snapper sdiff compares files at the sentence level. Whitespace reflow produces zero diff -- only actual content changes appear.",
    sdiff_scenario: "Same one-word change, but diffed at sentence level:",
    sdiff_reflow: "Paragraph reflow (rewrapping the same text) produces no diff at all:",
    features_label: "Features",
    features_title: "Built for academic workflows",
    features_subtitle: "Everything you need to adopt semantic line breaks across your paper writing toolchain.",
    feat1_title: "Format-aware parsing",
    feat1_desc: "Understands Org-mode, LaTeX, Markdown, RST, and plaintext. Code blocks, math environments, tables, directives, and drawers pass through untouched.",
    feat2_title: "Abbreviation-aware",
    feat2_desc: "Handles Dr., Fig., Eq., e.g., i.e., et al. and 80+ more. Add project-specific abbreviations via .snapperrc.toml.",
    feat3_title: "Pre-commit hook",
    feat3_desc: "Ships with .pre-commit-hooks.yaml. Enforce semantic line breaks automatically on every commit across your team.",
    feat4_title: "Editor integration",
    feat4_desc: "Stdin/stdout interface works with Emacs Apheleia, vim, and any editor that supports external formatters. Format on save.",
    feat5_title: "Vale style package",
    feat5_desc: "Bundled vale rules flag lines with multiple sentences in your editor. Use --check for precise CI enforcement.",
    feat6_title: "Git smudge/clean filter",
    feat6_desc: "Auto-format on commit, transparent to collaborators. Add to .gitattributes and formatting happens silently.",
    feat7_title: "Neural sentence detection",
    feat7_desc: "Optional ML-based splitting via nnsplit for 9 languages. Use --neural --lang de for non-English text where rules fall short.",
    feat8_title: "Skip regions with pragmas",
    feat8_desc: "Mark hand-formatted text with snapper:off / snapper:on comments. Poetry, aligned text, and ASCII art pass through untouched.",
    feat9_title: "One-command setup",
    feat9_desc: "snapper init detects your formats and generates config, pre-commit hooks, gitattributes, and editor snippets.",
    install_label: "Get started",
    install_title: "Install in seconds",
    footer_provided: "provided by"
  },
  is: {
    hero_title: "Skiptu texta i hreinar, <accent>diff-vaenlegar</accent> linur",
    hero_tagline: "Hratt og snidvitandi linuskiptitol fyrir fraedigreinar og samvinnu i ritun.",
    cta_github: "Skoda a GitHub",
    cta_docs: "Skjolun",
    demo_label: "Sjadu i verki",
    demo_title: "Ein setning, ein lina",
    demo_subtitle: "Hedbundin linuumbrot fela breytingar i malsgreibarvidum diff. Snapper brydur vid setningamark svo hver breyting sjest greinilega.",
    tab_before_after: "Fyrir og eftir",
    tab_diff: "Git Diff",
    tab_sdiff: "Setningamunur",
    pane_before: "Fyrir (hedbundin umbrot)",
    pane_after: "Eftir (snapper)",
    sdiff_header: "snapper sdiff ber saman skrar a setningarstigi. Endurumbrot gefa engan mun -- adeins raunverulegar efnisbreytingar birtast.",
    sdiff_scenario: "Sama eins ords breyting, en borid saman a setningarstigi:",
    sdiff_reflow: "Endurumbrot malsgreinar (sama texti, odruvis umbrotid) gefur engan mun:",
    features_label: "Eiginleikar",
    features_title: "Hannad fyrir fraedistorf",
    features_subtitle: "Allt sem tharf til ad taka upp merkingarleg linuskipti i ritverkfaeredinu thinu.",
    feat1_title: "Snidvitandi thattun",
    feat1_desc: "Skilur Org-mode, LaTeX, Markdown og hreinan texta. Kodablokkar, staedfraediumhverfi, toflur og skuffur fara i gegn obreyttar.",
    feat2_title: "Skammstofunarvitandi",
    feat2_desc: "Medhondlar Dr., mynd, jofnu, t.d., th.e., o.fl. og 80+ i vidbbot. Baettu vid verkefnasertaekum skammstofunum i .snapperrc.toml.",
    feat3_title: "Pre-commit hook",
    feat3_desc: "Kemur med .pre-commit-hooks.yaml. Framfylgdu merkingarlegum linuskiptum sjalfvirkt vid hverja commit.",
    feat4_title: "Ritilssamthaetting",
    feat4_desc: "Stdin/stdout tengi virkar med Emacs Apheleia, vim og hvada ritil sem stydur ytri snidtol. Snidad vid vistun.",
    feat5_title: "Vale stillpakki",
    feat5_desc: "Innbyggdar vale-reglur merkja linur med morgum setningum i ritlinum. Notid --check fyrir naekvaema CI-eftirfylgni.",
    feat6_title: "Git smudge/clean sia",
    feat6_desc: "Sjalfvirk snidmotun vid commit, gaegnsa samstarfsmonnum. Baettu vid .gitattributes og snidmotun gerist thegjandi.",
    feat7_title: "Taugakerfis-setningargreining",
    feat7_desc: "Valfrialls ML-greining med nnsplit fyrir 9 tungumal. Notadu --neural --lang de fyrir texta a odrum tungumaalum.",
    feat8_title: "Sleppa svaedum med pragma",
    feat8_desc: "Merktu handsnidinn texta med snapper:off / snapper:on. Ljod, jofnudur texti og ASCII-myndir fara i gegn obreyttar.",
    feat9_title: "Uppsetning med einni skipun",
    feat9_desc: "snapper init greinir snidin thin og byr til stillingar, pre-commit hooks, gitattributes og ritilsuppskriftir.",
    install_label: "Byrjadu",
    install_title: "Uppsetning a nokkrum sekundum",
    footer_provided: "gert af"
  },
  pl: {
    hero_title: "Podziel tekst na czyste, <accent>diff-przyjazne</accent> linie",
    hero_tagline: "Szybki formater semantycznych podzialow wierszy dla artykulow naukowych i wspolpracy redakcyjnej.",
    cta_github: "Zobacz na GitHub",
    cta_docs: "Dokumentacja",
    demo_label: "Zobacz w akcji",
    demo_title: "Jedno zdanie, jedna linia",
    demo_subtitle: "Tradycyjne zawijanie ukrywa zmiany w diff-ach obejmujacych cale akapity. Snapper lamie na granicach zdan -- kazda edycja pokazuje dokladnie co sie zmienilo.",
    tab_before_after: "Przed i po",
    tab_diff: "Git Diff",
    tab_sdiff: "Diff zdan",
    pane_before: "Przed (tradycyjne zawijanie)",
    pane_after: "Po (snapper)",
    sdiff_header: "snapper sdiff porownuje pliki na poziomie zdan. Zmiana zawijania daje zerowy diff -- widoczne sa tylko rzeczywiste zmiany tresci.",
    sdiff_scenario: "Ta sama jednowyrazowa zmiana, ale porownana na poziomie zdan:",
    sdiff_reflow: "Przeformatowanie akapitu (ten sam tekst, inne zawijanie) nie daje zadnego diff-a:",
    features_label: "Funkcje",
    features_title: "Stworzony dla pracy naukowej",
    features_subtitle: "Wszystko czego potrzebujesz aby wdrozyc semantyczne podzialy wierszy w swoim procesie pisania.",
    feat1_title: "Rozpoznawanie formatu",
    feat1_desc: "Rozumie Org-mode, LaTeX, Markdown i zwykly tekst. Bloki kodu, srodowiska matematyczne, tabele i szuflady przechodza bez zmian.",
    feat2_title: "Obsluga skrotow",
    feat2_desc: "Obsluguje Dr., Rys., Row., np., tj., et al. i ponad 80 innych. Dodaj skroty specyficzne dla projektu w .snapperrc.toml.",
    feat3_title: "Hook pre-commit",
    feat3_desc: "Zawiera .pre-commit-hooks.yaml. Wymuszaj semantyczne podzialy wierszy automatycznie przy kazdym zatwierdzeniu.",
    feat4_title: "Integracja z edytorem",
    feat4_desc: "Interfejs stdin/stdout dziala z Emacs Apheleia, vim i kazdym edytorem obslugujacym zewnetrzne formatery. Formatuj przy zapisie.",
    feat5_title: "Pakiet stylow vale",
    feat5_desc: "Dolaczone reguly vale oznaczaja linie z wieloma zdaniami w edytorze. Uzyj --check do precyzyjnego wymuszania w CI.",
    feat6_title: "Filtr git smudge/clean",
    feat6_desc: "Automatyczne formatowanie przy zatwierdzeniu, przezroczyste dla wspolpracownikow. Dodaj do .gitattributes -- formatowanie dzieje sie cicho.",
    feat7_title: "Neuronowe wykrywanie zdan",
    feat7_desc: "Opcjonalne dzielenie ML przez nnsplit dla 9 jezykow. Uzyj --neural --lang de dla tekstu nieanglojezycznego.",
    feat8_title: "Pomijanie regionow z pragmami",
    feat8_desc: "Oznacz recznie sformatowany tekst za pomoca snapper:off / snapper:on. Poezja, wyrownany tekst i ASCII art przechodza bez zmian.",
    feat9_title: "Konfiguracja jedna komenda",
    feat9_desc: "snapper init wykrywa formaty i generuje konfiguracje, hooki pre-commit, gitattributes i snippety dla edytora.",
    install_label: "Rozpocznij",
    install_title: "Instalacja w kilka sekund",
    footer_provided: "dostarczone przez"
  }
};
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let lang = "en";
    const t = derived(() => I18N[lang] || I18N.en);
    const githubIcon = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22"/></svg>`;
    const docsIcon = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"/><path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"/></svg>`;
    const featureIcons = [
      `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M16 13H8M16 17H8M10 9H8"/></svg>`,
      `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>`,
      `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77"/></svg>`,
      `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/></svg>`,
      `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 20h9M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>`,
      `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>`,
      `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2a4 4 0 0 1 4 4c0 1.1-.5 2.1-1.3 2.8l-.7.6V12h-4V9.4l-.7-.6A4 4 0 0 1 12 2z"/><path d="M9 12h6M10 15h4M10 18h4"/></svg>`,
      `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6L6 18M6 6l12 12"/></svg>`,
      `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 22h16a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v16a2 2 0 0 1-2 2zm0 0a2 2 0 0 1-2-2v-9c0-1.1.9-2 2-2h2"/></svg>`
    ];
    const features = derived(() => Array.from({ length: 9 }, (_, i) => ({
      icon: featureIcons[i],
      title: t()[`feat${i + 1}_title`] || "",
      description: t()[`feat${i + 1}_desc`] || ""
    })));
    const installOptions = [
      {
        title: "Cargo",
        lines: [
          { text: "# pre-built binary (fastest)", type: "comment" },
          { text: "cargo binstall snapper-fmt", type: "cmd" },
          { text: "# or compile from source", type: "comment" },
          { text: "cargo install snapper-fmt", type: "cmd" }
        ]
      },
      {
        title: "Shell / Homebrew",
        lines: [
          { text: "# one-liner (Linux/macOS)", type: "comment" },
          {
            text: "curl -LsSf https://github.com/TurtleTech-ehf/snapper/releases/latest/download/snapper-fmt-installer.sh | sh",
            type: "cmd"
          },
          { text: "# or Homebrew", type: "comment" },
          {
            text: "brew install TurtleTech-ehf/tap/snapper-fmt",
            type: "cmd"
          }
        ]
      },
      {
        title: "pip / Nix",
        lines: [
          { text: "# pip (with maturin wheels)", type: "comment" },
          { text: "pip install snapper-fmt", type: "cmd" },
          { text: "# Nix", type: "comment" },
          { text: "nix build github:TurtleTech-ehf/snapper", type: "cmd" }
        ]
      },
      {
        title: "Pre-commit",
        lines: [
          { text: "# .pre-commit-config.yaml", type: "comment" },
          {
            text: "- repo: https://github.com/TurtleTech-ehf/snapper",
            type: "flag"
          },
          { text: "  rev: v0.3.2", type: "flag" },
          { text: "  hooks:", type: "flag" },
          { text: "    - id: snapper", type: "flag" }
        ]
      }
    ];
    NoiseTexture($$renderer2, {});
    $$renderer2.push(`<!----> `);
    DarkModeToggle($$renderer2, {});
    $$renderer2.push(`<!----> `);
    LangSelector($$renderer2, {
      languages: [
        { code: "en", label: "EN" },
        { code: "is", label: "IS" },
        { code: "pl", label: "PL" }
      ]
    });
    $$renderer2.push(`<!----> `);
    HeroSection($$renderer2, {
      logoSrc: "/snapper_logo.png",
      logoAlt: "snapper - semantic line break formatter",
      title: "Snap your prose into clean, diff-friendly lines",
      accentWord: "diff-friendly",
      tagline: t().hero_tagline,
      primaryCta: {
        text: t().cta_github,
        href: "https://github.com/TurtleTech-ehf/snapper",
        icon: githubIcon
      },
      secondaryCta: { text: t().cta_docs, href: "docs/", icon: docsIcon }
    });
    $$renderer2.push(`<!----> <section class="demo-section svelte-1uha8ag"><div class="container">`);
    ScrollReveal($$renderer2, {
      children: ($$renderer3) => {
        SectionHeader($$renderer3, {
          label: t().demo_label,
          title: t().demo_title,
          subtitle: t().demo_subtitle
        });
      }
    });
    $$renderer2.push(`<!----> `);
    ScrollReveal($$renderer2, {
      children: ($$renderer3) => {
        DemoSection($$renderer3, { t: t() });
      }
    });
    $$renderer2.push(`<!----></div></section> <section class="features-section svelte-1uha8ag"><div class="container">`);
    ScrollReveal($$renderer2, {
      children: ($$renderer3) => {
        SectionHeader($$renderer3, {
          label: t().features_label,
          title: t().features_title,
          subtitle: t().features_subtitle
        });
      }
    });
    $$renderer2.push(`<!----> `);
    ScrollReveal($$renderer2, {
      children: ($$renderer3) => {
        FeatureGrid($$renderer3, { features: features() });
      }
    });
    $$renderer2.push(`<!----></div></section> <section class="install-section svelte-1uha8ag"><div class="container">`);
    ScrollReveal($$renderer2, {
      children: ($$renderer3) => {
        SectionHeader($$renderer3, { label: t().install_label, title: t().install_title });
      }
    });
    $$renderer2.push(`<!----> `);
    ScrollReveal($$renderer2, {
      children: ($$renderer3) => {
        InstallCards($$renderer3, { options: installOptions });
      }
    });
    $$renderer2.push(`<!----></div></section> `);
    ProductFooter($$renderer2, {
      license: "MIT License",
      repoUrl: "https://github.com/TurtleTech-ehf/snapper"
    });
    $$renderer2.push(`<!---->`);
  });
}
export {
  _page as default
};
