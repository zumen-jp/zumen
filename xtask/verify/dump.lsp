;; 検証キット（#503）の図面を、AutoCAD で読んだ値として 1 行ずつ書き出す。
;; 使い方: AutoCAD で図面を開き、(load "dump.lsp") (zv-dump "C:/zumen-verify/out.txt")
;; 画面なしの accoreconsole でも動く（vla の関数を使わない）。check_autocad.py は、この中身をスクリプトに埋めて流す
;; ので、注記のほかは ASCII だけで書き、1 行を 200 文字までにする。
;; 1 行目は「PDMODE=値<TAB>PDSIZE=値」。
;; 表の行は「@LAYER（@LTYPE・@STYLE）<TAB>ハンドル<TAB>名前<TAB>コード=値...」。
;; 図形の行は「ハンドル<TAB>種類<TAB>画層<TAB>コード=値...」。モデル空間と用紙の図形が先に並ぶ。
;; 「@BLOCK<TAB>ブロックのレコードのハンドル<TAB>名前」の行の後に、そのブロックの定義の図形が並ぶ。
;; 最後の行は「@END<TAB>図形の行の数」。この行がなければ、書き出しは途中で止まっている。
;; 点は「X,Y」（210 の押し出しの向きだけ「X,Y,Z」）。角度（50・51）は度に直して書く。
;; 文字は、制御文字を「^」と英字（タブは ^I）に、「^」を「^ 」に直して書く（DXF の書き方と同じ。zumen の読みも、
;; 文字をこの書き方で持つ）。
(defun zv-num (v) (rtos v 2 12))
(defun zv-str (s / out)
  (if (vl-some '(lambda (c) (or (< c 32) (= c 94))) (vl-string->list s))
    (progn
      (setq out "")
      (foreach c (vl-string->list s)
        (setq out (strcat out (cond ((= c 94) "^ ")
                                    ((< c 32) (strcat "^" (chr (+ c 64))))
                                    (t (chr c))))))
      out)
    s))
(defun zv-text (code v)
  (cond ((and (listp v) (numberp (car v)))
         (strcat (zv-num (car v)) "," (zv-num (cadr v))
                 (if (= code 210) (strcat "," (zv-num (caddr v))) "")))
        ((= (type v) 'REAL) (zv-num (if (member code '(50 51)) (* 180.0 (/ v pi)) v)))
        ((= (type v) 'INT) (itoa v))
        ((= (type v) 'STR) (zv-str v))
        ((= (type v) 'ENAME) (cdr (assoc 5 (entget v))))
        (t "?")))
(setq zv-codes '(1 2 3 6 7 10 11 12 13 14 15 38 40 41 42 43 44 45 48 50 51 60 62 66 67
                 70 71 72 73 74 90 210 370 410 420))
(setq zv-table-codes '(3 4 6 40 41 42 49 50 62 70 71 72 73 74 290 370 420))
(defun zv-fields (d codes / s)
  (setq s "")
  (foreach pair d
    (if (member (car pair) codes)
      (setq s (strcat s "\t" (itoa (car pair)) "=" (zv-text (car pair) (cdr pair))))))
  s)
(defun zv-entity (e f / d)
  (setq d (entget e))
  (write-line (strcat (cdr (assoc 5 d)) "\t" (cdr (assoc 0 d)) "\t" (zv-str (cdr (assoc 8 d)))
                      (zv-fields d zv-codes))
              f)
  (setq zv-count (1+ zv-count)))
(defun zv-table (name f / r d)
  (setq r (tblnext name T))
  (while r
    (setq d (entget (tblobjname name (cdr (assoc 2 r)))))
    (write-line (strcat "@" name "\t" (cdr (assoc 5 d)) "\t" (zv-str (cdr (assoc 2 d)))
                        (zv-fields d zv-table-codes))
                f)
    (setq r (tblnext name))))
(defun zv-blocks (f / r d e)
  (setq r (tblnext "BLOCK" T))
  (while r
    (if (not (wcmatch (strcase (cdr (assoc 2 r))) "`*MODEL_SPACE,`*PAPER_SPACE*"))
      (progn
        (setq d (entget (tblobjname "BLOCK" (cdr (assoc 2 r)))))
        (write-line (strcat "@BLOCK\t" (cdr (assoc 5 (entget (cdr (assoc 330 d))))) "\t"
                            (zv-str (cdr (assoc 2 r))) (zv-fields r '(10 70)))
                    f)
        (setq e (cdr (assoc -2 r)))
        (while e
          (zv-entity e f)
          (setq e (entnext e)))))
    (setq r (tblnext "BLOCK"))))
(defun zv-dump (path / f e)
  (setq f (if (getvar "LISPSYS") (open path "w" "utf8") (open path "w")))
  (setq zv-count 0)
  (write-line (strcat "PDMODE=" (itoa (getvar "PDMODE")) "\tPDSIZE=" (zv-num (getvar "PDSIZE"))) f)
  (foreach name '("LAYER" "LTYPE" "STYLE") (zv-table name f))
  (setq e (entnext))
  (while e
    (zv-entity e f)
    (setq e (entnext e)))
  (zv-blocks f)
  (write-line (strcat "@END\t" (itoa zv-count)) f)
  (close f)
  (princ (strcat "\nzv-dump: " (itoa zv-count)))
  (princ))
