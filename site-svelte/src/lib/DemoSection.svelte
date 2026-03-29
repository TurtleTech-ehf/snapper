<script lang="ts">
  interface Props {
    t: Record<string, string>;
  }

  let { t }: Props = $props();
  let activeTab = $state('split');
</script>

<div class="demo-wrapper">
  <div class="demo-tabs">
    <button class="demo-tab first" class:active={activeTab === 'split'} onclick={() => activeTab = 'split'}>{t.tab_before_after}</button>
    <button class="demo-tab" class:active={activeTab === 'diff'} onclick={() => activeTab = 'diff'}>{t.tab_diff}</button>
    <button class="demo-tab last" class:active={activeTab === 'sdiff'} onclick={() => activeTab = 'sdiff'}>{t.tab_sdiff}</button>
  </div>

  <div class="demo-panel" class:active={activeTab === 'split'}>
    <div class="split-view">
      <div class="split-pane pane-before">
        <span class="split-pane-label">{t.pane_before}</span>
We present a method for formatting prose documents with semantic line breaks, where each sentence occupies its own line. This approach dramatically reduces noise in version control diffs when multiple authors collaborate on a manuscript. Our tool handles LaTeX, Org-mode, Markdown, and plaintext with format-aware parsing that preserves structural elements.</div>
      <div class="split-pane pane-after">
        <span class="split-pane-label">{t.pane_after}</span>
        <span class="sentence">We present a method for formatting prose documents with semantic line breaks, where each sentence occupies its own line.</span>
        <span class="sentence">This approach dramatically reduces noise in version control diffs when multiple authors collaborate on a manuscript.</span>
        <span class="sentence">Our tool handles LaTeX, Org-mode, Markdown, and plaintext with format-aware parsing that preserves structural elements.</span>
      </div>
    </div>
  </div>

  <div class="demo-panel" class:active={activeTab === 'diff'}>
    <div class="diff-view">
      <div class="diff-scenario">
        <span class="scenario-label">Without snapper: change one word, diff shows entire paragraph</span>
        <span class="diff-file">--- a/paper.org</span>
        <span class="diff-file">+++ b/paper.org</span>
        <span class="diff-hunk">@@ -1,3 +1,3 @@</span>
<span class="diff-rm">-We present a method for formatting prose documents with semantic line breaks, where each sentence occupies its own line. This approach dramatically reduces noise in version control diffs when multiple authors collaborate on a manuscript. Our tool handles LaTeX, Org-mode, Markdown, and plaintext with format-aware parsing that preserves structural elements.</span>
<span class="diff-add">+We present a method for formatting prose documents with semantic line breaks, where each sentence occupies its own line. This approach <span class="diff-highlight">significantly</span> reduces noise in version control diffs when multiple authors collaborate on a manuscript. Our tool handles LaTeX, Org-mode, Markdown, and plaintext with format-aware parsing that preserves structural elements.</span>
      </div>
      <div class="diff-scenario">
        <span class="scenario-label">With snapper: same change, diff shows one line</span>
        <span class="diff-file">--- a/paper.org</span>
        <span class="diff-file">+++ b/paper.org</span>
        <span class="diff-hunk">@@ -1,3 +1,3 @@</span>
<span class="diff-ctx"> We present a method for formatting prose documents with semantic line breaks, where each sentence occupies its own line.</span>
<span class="diff-rm">-This approach dramatically reduces noise in version control diffs when multiple authors collaborate on a manuscript.</span>
<span class="diff-add">+This approach <span class="diff-highlight">significantly</span> reduces noise in version control diffs when multiple authors collaborate on a manuscript.</span>
<span class="diff-ctx"> Our tool handles LaTeX, Org-mode, Markdown, and plaintext with format-aware parsing that preserves structural elements.</span>
      </div>
    </div>
  </div>

  <div class="demo-panel" class:active={activeTab === 'sdiff'}>
    <div class="diff-view">
      <div class="diff-header">{t.sdiff_header}</div>
      <div class="diff-scenario">
        <span class="scenario-label">{t.sdiff_scenario}</span>
        <span class="diff-file">$ snapper sdiff paper_v1.org paper_v2.org</span>
        <span class="diff-hunk">@@ -1,3 +1,3 @@</span>
<span class="diff-ctx"> We present a method for formatting prose documents with semantic line breaks, where each sentence occupies its own line.</span>
<span class="diff-rm">-This approach dramatically reduces noise in version control diffs when multiple authors collaborate on a manuscript.</span>
<span class="diff-add">+This approach <span class="diff-highlight">significantly</span> reduces noise in version control diffs when multiple authors collaborate on a manuscript.</span>
<span class="diff-ctx"> Our tool handles LaTeX, Org-mode, Markdown, and plaintext with format-aware parsing that preserves structural elements.</span>
      </div>
      <div class="diff-scenario">
        <span class="scenario-label">{t.sdiff_reflow}</span>
        <span class="diff-file">$ snapper sdiff wrapped_at_80.org semantic_breaks.org</span>
        <span class="diff-ctx" style="color: #6a6; font-style: italic;">No sentence-level differences.</span>
      </div>
    </div>
  </div>
</div>

<style>
  .demo-tabs { display: flex; }

  .demo-tab {
    flex: 1;
    padding: 0.75rem 1.5rem;
    font-family: var(--sans);
    font-size: 0.85rem;
    font-weight: 600;
    background: transparent;
    border: 2px solid var(--green-dark);
    border-right: none;
    color: var(--green-dark);
    cursor: pointer;
    transition: background 0.2s, color 0.2s;
  }
  .demo-tab.first { border-radius: 8px 0 0 0; }
  .demo-tab.last { border-right: 2px solid var(--green-dark); border-radius: 0 8px 0 0; }
  .demo-tab.active { background: var(--green-dark); color: white; }
  .demo-tab:not(.active):hover { background: rgba(45,90,61,0.08); }

  .demo-panel {
    display: none;
    border: 2px solid var(--green-dark);
    border-top: none;
    border-radius: 0 0 12px 12px;
    overflow: hidden;
  }
  .demo-panel.active { display: block; }

  .split-view { display: grid; grid-template-columns: 1fr 1fr; }
  .split-pane {
    padding: 1.5rem;
    font-size: 0.9rem;
    line-height: 1.7;
    white-space: pre-wrap;
    font-family: var(--mono);
  }
  .pane-before {
    background: white;
    border-right: 2px solid var(--green-dark);
    color: var(--text);
  }
  .pane-after { background: var(--cream-light); }
  .split-pane-label {
    display: block;
    font-family: var(--sans);
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    margin-bottom: 1rem;
    color: var(--text-muted);
  }
  .pane-after .split-pane-label { color: var(--green-mid); }

  .sentence {
    display: block;
    padding: 0.25rem 0;
    border-left: 3px solid transparent;
    padding-left: 0.75rem;
  }
  .sentence:nth-child(odd) { border-left-color: var(--green-light); }
  .sentence:nth-child(even) { border-left-color: var(--coral); }

  .diff-view {
    background: var(--code-bg);
    color: #d4d4d4;
    font-family: var(--mono);
    font-size: 0.8rem;
    line-height: 1.7;
    padding: 1.25rem;
    overflow-x: auto;
  }
  .diff-header {
    color: var(--text-muted);
    font-family: var(--sans);
    font-size: 0.85rem;
    margin-bottom: 1.25rem;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid rgba(255,255,255,0.06);
  }
  .diff-scenario { margin-bottom: 1.5rem; }
  .diff-scenario:last-child { margin-bottom: 0; }
  .scenario-label {
    display: block;
    color: var(--yellow);
    font-family: var(--sans);
    font-size: 0.75rem;
    font-weight: 600;
    margin-bottom: 0.5rem;
  }
  .diff-file { display: block; color: #888; }
  .diff-hunk { display: block; color: #9cdcfe; margin: 0.25rem 0; }
  .diff-add { display: block; color: var(--diff-add-strong, #a8e6a0); background: var(--diff-add, #d4f5d0); background: rgba(107,175,91,0.1); padding: 0.1rem 0.5rem; white-space: pre-wrap; }
  .diff-rm { display: block; color: var(--diff-rm-strong, #f8b0b0); background: rgba(255,101,93,0.1); padding: 0.1rem 0.5rem; white-space: pre-wrap; }
  .diff-ctx { display: block; padding: 0.1rem 0.5rem; white-space: pre-wrap; }
  .diff-highlight { background: rgba(255,101,93,0.25); padding: 0.1rem 0.2rem; border-radius: 2px; }

  :global([data-theme="dark"]) .pane-before { background: #152218; }
  :global([data-theme="dark"]) .demo-tab { border-color: var(--green-dark); color: var(--green-dark); }
  :global([data-theme="dark"]) .demo-tab.active { background: var(--green-dark); color: var(--bg); }
  :global([data-theme="dark"]) .demo-panel { border-color: var(--green-dark); }
  :global([data-theme="dark"]) .pane-before { border-right-color: var(--green-dark); }

  @media (max-width: 768px) {
    .split-view { grid-template-columns: 1fr; }
    .pane-before { border-right: none; border-bottom: 2px solid var(--green-dark); }
  }
</style>
