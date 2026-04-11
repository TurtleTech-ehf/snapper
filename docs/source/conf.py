import os

project = "snapper"
copyright = '2026--present, <a href="https://rgoswami.me">Rohit Goswami</a>'
author = "Rohit Goswami"
release = "0.7.7"
html_logo = "../../branding/logo/snapper_logo.png"

extensions = [
    "myst_parser",
    "sphinx_sitemap",
    "sphinx.ext.intersphinx",
]

templates_path = ["_templates"]
exclude_patterns = []

# -- i18n (Sphinx gettext + sphinx-intl) --
locale_dirs = ["../locale/"]
gettext_compact = False
language = os.environ.get("SPHINX_LANGUAGE", "en")

html_theme = "shibuya"
html_static_path = ["_static"]
html_favicon = "_static/favicon.ico"
html_css_files = ["custom.css"]

html_baseurl = "https://snapper.turtletech.us/docs/"

html_theme_options = {
    "accent_color": "teal",
    "dark_code": True,
    "globaltoc_expand_depth": 2,
    "github_url": "https://github.com/TurtleTech-ehf/snapper",
    "logo_url": "https://snapper.turtletech.us/",
    "og_description": "Semantic line break formatter for Org-mode, LaTeX, Markdown, and plaintext. Clean git diffs for collaborative academic writing.",
    "og_image": "https://snapper.turtletech.us/snapper_logo.png",
}

# Shibuya native language selector (renders in nav bar automatically)
html_context = {
    "source_type": "github",
    "source_user": "TurtleTech-ehf",
    "source_repo": "snapper",
    "source_version": "main",
    "source_docs_path": "/docs/source/",
    "languages": [
        ("English", "/docs/%s/", "en"),
        ("Islenska", "/docs/is/%s/", "is"),
        ("Polski", "/docs/pl/%s/", "pl"),
    ],
}

myst_enable_extensions = [
    "colon_fence",
    "deflist",
]

intersphinx_mapping = {}

source_suffix = {
    ".rst": "restructuredtext",
    ".md": "markdown",
}
