<script lang="ts">
  // Import from ttech-ui source (symlinked in node_modules)
  import HeroSection from '@turtletech/ui/src/lib/components/HeroSection/HeroSection.svelte';
  import SectionHeader from '@turtletech/ui/src/lib/components/SectionHeader/SectionHeader.svelte';
  import FeatureGrid from '@turtletech/ui/src/lib/components/FeatureGrid/FeatureGrid.svelte';
  import InstallCards from '@turtletech/ui/src/lib/components/InstallCards/InstallCards.svelte';
  import ProductFooter from '@turtletech/ui/src/lib/components/ProductFooter/ProductFooter.svelte';
  import DarkModeToggle from '@turtletech/ui/src/lib/components/DarkModeToggle/DarkModeToggle.svelte';
  import LangSelector from '@turtletech/ui/src/lib/components/LangSelector/LangSelector.svelte';
  import NoiseTexture from '@turtletech/ui/src/lib/components/NoiseTexture/NoiseTexture.svelte';
  import ScrollReveal from '@turtletech/ui/src/lib/components/ScrollReveal/ScrollReveal.svelte';
  import DemoSection from '$lib/DemoSection.svelte';
  import { I18N } from '$lib/i18n';

  let lang = $state('en');
  const t = $derived(I18N[lang] || I18N.en);

  // SVG icons for CTAs
  const githubIcon = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22"/></svg>`;
  const docsIcon = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"/><path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"/></svg>`;

  // Feature icons (inline SVGs matching original)
  const featureIcons = [
    `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M16 13H8M16 17H8M10 9H8"/></svg>`,
    `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>`,
    `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77"/></svg>`,
    `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/></svg>`,
    `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 20h9M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>`,
    `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>`,
    `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2a4 4 0 0 1 4 4c0 1.1-.5 2.1-1.3 2.8l-.7.6V12h-4V9.4l-.7-.6A4 4 0 0 1 12 2z"/><path d="M9 12h6M10 15h4M10 18h4"/></svg>`,
    `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6L6 18M6 6l12 12"/></svg>`,
    `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 22h16a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v16a2 2 0 0 1-2 2zm0 0a2 2 0 0 1-2-2v-9c0-1.1.9-2 2-2h2"/></svg>`,
  ];

  const features = $derived(
    Array.from({ length: 9 }, (_, i) => ({
      icon: featureIcons[i],
      title: t[`feat${i + 1}_title`] || '',
      description: t[`feat${i + 1}_desc`] || '',
    })),
  );

  const installOptions = [
    {
      title: 'Cargo',
      lines: [
        { text: '# pre-built binary (fastest)', type: 'comment' as const },
        { text: 'cargo binstall snapper-fmt', type: 'cmd' as const },
        { text: '# or compile from source', type: 'comment' as const },
        { text: 'cargo install snapper-fmt', type: 'cmd' as const },
      ],
    },
    {
      title: 'Shell / Homebrew',
      lines: [
        { text: '# one-liner (Linux/macOS)', type: 'comment' as const },
        { text: 'curl -LsSf https://github.com/TurtleTech-ehf/snapper/releases/latest/download/snapper-fmt-installer.sh | sh', type: 'cmd' as const },
        { text: '# or Homebrew', type: 'comment' as const },
        { text: 'brew install TurtleTech-ehf/tap/snapper-fmt', type: 'cmd' as const },
      ],
    },
    {
      title: 'pip / Nix',
      lines: [
        { text: '# pip (with maturin wheels)', type: 'comment' as const },
        { text: 'pip install snapper-fmt', type: 'cmd' as const },
        { text: '# Nix', type: 'comment' as const },
        { text: 'nix build github:TurtleTech-ehf/snapper', type: 'cmd' as const },
      ],
    },
    {
      title: 'Pre-commit',
      lines: [
        { text: '# .pre-commit-config.yaml', type: 'comment' as const },
        { text: '- repo: https://github.com/TurtleTech-ehf/snapper', type: 'flag' as const },
        { text: '  rev: v0.3.2', type: 'flag' as const },
        { text: '  hooks:', type: 'flag' as const },
        { text: '    - id: snapper', type: 'flag' as const },
      ],
    },
  ];
</script>

<NoiseTexture />
<DarkModeToggle storageKey="snapper-theme" />
<LangSelector
  languages={[
    { code: 'en', label: 'EN' },
    { code: 'is', label: 'IS' },
    { code: 'pl', label: 'PL' },
  ]}
  storageKey="snapper-lang"
  onchange={(l) => (lang = l)}
/>

<HeroSection
  logoSrc="/snapper_logo.png"
  logoAlt="snapper - semantic line break formatter"
  title="Snap your prose into clean, diff-friendly lines"
  accentWord="diff-friendly"
  tagline={t.hero_tagline}
  primaryCta={{ text: t.cta_github, href: 'https://github.com/TurtleTech-ehf/snapper', icon: githubIcon }}
  secondaryCta={{ text: t.cta_docs, href: 'docs/', icon: docsIcon }}
/>

<section class="demo-section">
  <div class="container">
    <ScrollReveal>
      <SectionHeader label={t.demo_label} title={t.demo_title} subtitle={t.demo_subtitle} />
    </ScrollReveal>
    <ScrollReveal>
      <DemoSection {t} />
    </ScrollReveal>
  </div>
</section>

<section class="features-section">
  <div class="container">
    <ScrollReveal>
      <SectionHeader label={t.features_label} title={t.features_title} subtitle={t.features_subtitle} />
    </ScrollReveal>
    <ScrollReveal>
      <FeatureGrid {features} />
    </ScrollReveal>
  </div>
</section>

<section class="install-section">
  <div class="container">
    <ScrollReveal>
      <SectionHeader label={t.install_label} title={t.install_title} />
    </ScrollReveal>
    <ScrollReveal>
      <InstallCards options={installOptions} />
    </ScrollReveal>
  </div>
</section>

<ProductFooter
  license="MIT License"
  repoUrl="https://github.com/TurtleTech-ehf/snapper"
/>

<style>
  .demo-section {
    background: linear-gradient(180deg, var(--bg) 0%, var(--cream-light) 100%);
  }
  .features-section {
    background: var(--cream-light);
    border-top: 1px solid transparent;
    background-clip: padding-box;
    border-image: linear-gradient(90deg, transparent, rgba(0,77,64,0.15), transparent) 1;
  }
  .install-section {
    background: var(--bg);
  }
</style>
