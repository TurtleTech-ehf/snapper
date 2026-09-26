;; Batch export org-mode files to RST for Sphinx
;; Usage: emacs --batch -l docs/export.el
;; Hermetic: clones ox-rst from source if not cached, no MELPA dependency.

(require 'ox-publish)

;; Clone ox-rst to a local cache dir if not present
(let* ((cache-dir (expand-file-name ".ox-rst" (file-name-directory load-file-name)))
       (ox-rst-file (expand-file-name "ox-rst.el" cache-dir)))
  (unless (file-exists-p ox-rst-file)
    (message "Cloning ox-rst...")
    (make-directory cache-dir t)
    (call-process "git" nil nil nil
                  "clone" "--depth" "1"
                  "https://github.com/msnoigrs/ox-rst.git" cache-dir))
  (add-to-list 'load-path cache-dir))

(require 'ox-rst)
(require 'ox-publish)

;; Define the Publishing Project
(setq org-publish-project-alist
      '(("sphinx-rst"
         :base-directory "./docs/orgmode/"
         :base-extension "org"
         :publishing-directory "./docs/source/"
         :publishing-function org-rst-publish-to-rst
         :recursive t
         :headline-levels 4
         :with-toc nil
         :section-numbers nil)))

;; Run the publish
(org-publish "sphinx-rst" t)
