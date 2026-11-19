;; Windows の AutoLISP から zumen の CLI を呼び、応答の JSON は jq で読む。
(vl-load-com)

(defun zumen-field (shell exe drawing op options field / process json parser output)
  (setq process
    (vlax-invoke-method shell 'Exec
      (strcat "\"" exe "\" " op " \"" drawing "\"" options)))
  (vlax-invoke-method (vlax-get-property process 'StdIn) 'Close)
  (setq json (vlax-invoke-method (vlax-get-property process 'StdOut) 'ReadAll))
  (if (= 0 (vlax-get-property process 'ExitCode))
    (progn
      (setq parser (vlax-invoke-method shell 'Exec (strcat "jq.exe -er " field)))
      (vlax-invoke-method (vlax-get-property parser 'StdIn) 'WriteLine json)
      (vlax-invoke-method (vlax-get-property parser 'StdIn) 'Close)
      (setq output (vlax-invoke-method (vlax-get-property parser 'StdOut) 'ReadAll))
      (if (= 0 (vlax-get-property parser 'ExitCode))
        (setq output (vl-string-right-trim "\r\n" output))
        (setq output nil))
      (vlax-release-object parser)))
  (vlax-release-object process)
  output)

(defun zumen-summary (exe drawing / shell format-name total)
  (setq shell (vlax-create-object "WScript.Shell"))
  (setq format-name (zumen-field shell exe drawing "open" "" ".result.format"))
  (if format-name
    (setq total (zumen-field shell exe drawing "entities" " --limit 1" ".result.total")))
  (vlax-release-object shell)
  (if (and format-name total)
    (strcat "format " format-name "\nentities " total "\n")
    (prompt "\nzumen or jq failed.")))
