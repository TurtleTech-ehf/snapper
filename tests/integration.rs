//! Every integration test in one binary. A file under tests/ built as its
//! own crate links the whole library, so `cargo test --release` would link
//! some 135 optimised binaries at once and can exhaust memory (#504); one
//! binary links the library once. `pandoc_parity_speed` keeps its own
//! process because it sets the pandoc cache variables other tests read.

#[path = "check_line_diagnostics.rs"]
mod check_line_diagnostics;
#[path = "cli_integration.rs"]
mod cli_integration;
#[path = "code_block_format.rs"]
mod code_block_format;
#[path = "code_block_reflow.rs"]
mod code_block_reflow;
#[path = "latex_enumerate_hang.rs"]
mod latex_enumerate_hang;
#[path = "latex_figure_table_captions.rs"]
mod latex_figure_table_captions;
#[path = "latex_ieeeeqnarray_xltabular.rs"]
mod latex_ieeeeqnarray_xltabular;
#[path = "latex_leftover_cmds.rs"]
mod latex_leftover_cmds;
#[path = "latex_more_envs.rs"]
mod latex_more_envs;
#[path = "latex_nicetabular.rs"]
mod latex_nicetabular;
#[path = "latex_non_prose_envs.rs"]
mod latex_non_prose_envs;
#[path = "latex_section_short_title.rs"]
mod latex_section_short_title;
#[path = "latex_tikz_pgf_envs.rs"]
mod latex_tikz_pgf_envs;
#[path = "latex_verbatim_envs.rs"]
mod latex_verbatim_envs;
#[path = "md_atx_heading_indent.rs"]
mod md_atx_heading_indent;
#[path = "md_compact_dl_markers.rs"]
mod md_compact_dl_markers;
#[path = "md_compact_dl_terms.rs"]
mod md_compact_dl_terms;
#[path = "md_definition_lists.rs"]
mod md_definition_lists;
#[path = "md_dollar_period.rs"]
mod md_dollar_period;
#[path = "md_empty_atx_heading.rs"]
mod md_empty_atx_heading;
#[path = "md_empty_list_item.rs"]
mod md_empty_list_item;
#[path = "md_example_list.rs"]
mod md_example_list;
#[path = "md_four_space_nested_list.rs"]
mod md_four_space_nested_list;
#[path = "md_gfm_alerts.rs"]
mod md_gfm_alerts;
#[path = "md_html_comment_indent.rs"]
mod md_html_comment_indent;
#[path = "md_html_type6_close.rs"]
mod md_html_type6_close;
#[path = "md_html_type7_close.rs"]
mod md_html_type7_close;
#[path = "md_light_table_interrupt.rs"]
mod md_light_table_interrupt;
#[path = "md_link_ref_dest_next_line.rs"]
mod md_link_ref_dest_next_line;
#[path = "md_link_ref_footnotes.rs"]
mod md_link_ref_footnotes;
#[path = "md_link_ref_title_next_line.rs"]
mod md_link_ref_title_next_line;
#[path = "md_link_tail_sentence.rs"]
mod md_link_tail_sentence;
#[path = "md_list_hang_code.rs"]
mod md_list_hang_code;
#[path = "md_list_item_lrd.rs"]
mod md_list_item_lrd;
#[path = "md_lrd_no_interrupt.rs"]
mod md_lrd_no_interrupt;
#[path = "md_multiline_setext.rs"]
mod md_multiline_setext;
#[path = "md_ol_opener_list_item.rs"]
mod md_ol_opener_list_item;
#[path = "md_ol_start_setext.rs"]
mod md_ol_start_setext;
#[path = "md_ol_ten_digit.rs"]
mod md_ol_ten_digit;
#[path = "md_pandoc_citation.rs"]
mod md_pandoc_citation;
#[path = "md_pandoc_line_block.rs"]
mod md_pandoc_line_block;
#[path = "md_quoted_definition_lists.rs"]
mod md_quoted_definition_lists;
#[path = "md_quoted_empty_list_item.rs"]
mod md_quoted_empty_list_item;
#[path = "md_quoted_footnote.rs"]
mod md_quoted_footnote;
#[path = "md_quoted_gfm_tables.rs"]
mod md_quoted_gfm_tables;
#[path = "md_quoted_html_blocks.rs"]
mod md_quoted_html_blocks;
#[path = "md_quoted_indented_code.rs"]
mod md_quoted_indented_code;
#[path = "md_quoted_lrd.rs"]
mod md_quoted_lrd;
#[path = "md_ref_link_interior_punct.rs"]
mod md_ref_link_interior_punct;
#[path = "md_shortcut_ref_label.rs"]
mod md_shortcut_ref_label;
#[path = "md_spanning_lrd_title.rs"]
mod md_spanning_lrd_title;
#[path = "md_yaml_front_matter.rs"]
mod md_yaml_front_matter;
#[path = "org_angle_links.rs"]
mod org_angle_links;
#[path = "org_brace_subscript.rs"]
mod org_brace_subscript;
#[path = "org_caption.rs"]
mod org_caption;
#[path = "org_clock_line.rs"]
mod org_clock_line;
#[path = "org_diary_sexp.rs"]
mod org_diary_sexp;
#[path = "org_dollar_space.rs"]
mod org_dollar_space;
#[path = "org_drawer_word_names.rs"]
mod org_drawer_word_names;
#[path = "org_dynamic_block.rs"]
mod org_dynamic_block;
#[path = "org_empty_list_item.rs"]
mod org_empty_list_item;
#[path = "org_export_snippet.rs"]
mod org_export_snippet;
#[path = "org_export_snippet_at.rs"]
mod org_export_snippet_at;
#[path = "org_file_token_punct.rs"]
mod org_file_token_punct;
#[path = "org_footnote_definitions.rs"]
mod org_footnote_definitions;
#[path = "org_hash_comment.rs"]
mod org_hash_comment;
#[path = "org_incomplete_begin.rs"]
mod org_incomplete_begin;
#[path = "org_inline_footnotes.rs"]
mod org_inline_footnotes;
#[path = "org_inline_src.rs"]
mod org_inline_src;
#[path = "org_item_tag.rs"]
mod org_item_tag;
#[path = "org_keyword_re.rs"]
mod org_keyword_re;
#[path = "org_latex_env_eol_closer.rs"]
mod org_latex_env_eol_closer;
#[path = "org_latex_fragment.rs"]
mod org_latex_fragment;
#[path = "org_line_breaks.rs"]
mod org_line_breaks;
#[path = "org_macros.rs"]
mod org_macros;
#[path = "org_planning_case.rs"]
mod org_planning_case;
#[path = "org_radio_targets.rs"]
mod org_radio_targets;
#[path = "org_special_block.rs"]
mod org_special_block;
#[path = "org_table_rule.rs"]
mod org_table_rule;
#[path = "org_timestamps.rs"]
mod org_timestamps;
#[path = "org_unicode_src_subscript.rs"]
mod org_unicode_src_subscript;
#[path = "org_unmatched_display_math.rs"]
mod org_unmatched_display_math;
#[path = "org_unmatched_drawer.rs"]
mod org_unmatched_drawer;
#[path = "org_unmatched_dynamic_block.rs"]
mod org_unmatched_dynamic_block;
#[path = "org_unmatched_opaque.rs"]
mod org_unmatched_opaque;
#[path = "org_verse.rs"]
mod org_verse;
#[path = "org_wrap_created_fn.rs"]
mod org_wrap_created_fn;
#[path = "org_wrap_created_link.rs"]
mod org_wrap_created_link;
#[path = "org_wrap_file_token.rs"]
mod org_wrap_file_token;
#[path = "pandoc_ast_backend.rs"]
mod pandoc_ast_backend;
#[path = "rst_abbrev_list_continuation.rs"]
mod rst_abbrev_list_continuation;
#[path = "rst_anonymous_targets.rs"]
mod rst_anonymous_targets;
#[path = "rst_bibliographic_abstract.rs"]
mod rst_bibliographic_abstract;
#[path = "rst_comment_blank.rs"]
mod rst_comment_blank;
#[path = "rst_compact_listlike_paragraph.rs"]
mod rst_compact_listlike_paragraph;
#[path = "rst_compact_note_directive.rs"]
mod rst_compact_note_directive;
#[path = "rst_container_directives.rs"]
mod rst_container_directives;
#[path = "rst_csv_table_title.rs"]
mod rst_csv_table_title;
#[path = "rst_definition_after_directive.rs"]
mod rst_definition_after_directive;
#[path = "rst_directive_title.rs"]
mod rst_directive_title;
#[path = "rst_doctest_blocks.rs"]
mod rst_doctest_blocks;
#[path = "rst_empty_doctest.rs"]
mod rst_empty_doctest;
#[path = "rst_empty_explicit_comment.rs"]
mod rst_empty_explicit_comment;
#[path = "rst_empty_field_list.rs"]
mod rst_empty_field_list;
#[path = "rst_empty_list_item.rs"]
mod rst_empty_list_item;
#[path = "rst_enumerators.rs"]
mod rst_enumerators;
#[path = "rst_footnotes_citations.rs"]
mod rst_footnotes_citations;
#[path = "rst_header_footer.rs"]
mod rst_header_footer;
#[path = "rst_interior_colon_field.rs"]
mod rst_interior_colon_field;
#[path = "rst_jinja_statements.rs"]
mod rst_jinja_statements;
#[path = "rst_line_block_directive.rs"]
mod rst_line_block_directive;
#[path = "rst_line_blocks.rs"]
mod rst_line_blocks;
#[path = "rst_literal_list_continuation.rs"]
mod rst_literal_list_continuation;
#[path = "rst_meta.rs"]
mod rst_meta;
#[path = "rst_nested_list_continuation.rs"]
mod rst_nested_list_continuation;
#[path = "rst_option_lists.rs"]
mod rst_option_lists;
#[path = "rst_overindented_list_continuation.rs"]
mod rst_overindented_list_continuation;
#[path = "rst_paren_list_continuation.rs"]
mod rst_paren_list_continuation;
#[path = "rst_parsed_literal.rs"]
mod rst_parsed_literal;
#[path = "rst_quoted_list_continuation.rs"]
mod rst_quoted_list_continuation;
#[path = "rst_quoted_literals.rs"]
mod rst_quoted_literals;
#[path = "rst_role_list_continuation.rs"]
mod rst_role_list_continuation;
#[path = "rst_same_line_note.rs"]
mod rst_same_line_note;
#[path = "rst_section_adornment.rs"]
mod rst_section_adornment;
#[path = "rst_simple_table_interior_blank.rs"]
mod rst_simple_table_interior_blank;
#[path = "rst_simple_table_top.rs"]
mod rst_simple_table_top;
#[path = "rst_substitution_refs.rs"]
mod rst_substitution_refs;
#[path = "rst_substitution_replace.rs"]
mod rst_substitution_replace;
#[path = "rst_two_space_bullet_marker.rs"]
mod rst_two_space_bullet_marker;
#[path = "safety_props.rs"]
mod safety_props;
#[path = "safety_splice.rs"]
mod safety_splice;
#[path = "same_line_lowercase_proper_noun.rs"]
mod same_line_lowercase_proper_noun;
#[path = "sentence_delim_props.rs"]
mod sentence_delim_props;
#[path = "snapper_19jf.rs"]
mod snapper_19jf;
