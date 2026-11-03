;; 確認の束（#503）の図面の図形を、AutoCAD で読んだ値として 1 行ずつ書き出す。
;; 使い方: AutoCAD で図面を開き、(load "dump.lsp") (zv-dump "C:/zumen-verify/out.txt")
;; 画面なしの accoreconsole でも動く（vla の関数を使わない）。角度（50・51）は度に直して書く。
;; 行は「ハンドル<TAB>種類<TAB>画層<TAB>コード=値...」。点は「X,Y」。文字は改行を除いてそのまま
(defun zv-num (v) (rtos v 2 12))
(defun zv-text (code v)
  (cond ((and (listp v) (numberp (car v))) (strcat (zv-num (car v)) "," (zv-num (cadr v))))
        ((= (type v) 'REAL) (zv-num (if (member code '(50 51)) (* 180.0 (/ v pi)) v)))
        ((= (type v) 'INT) (itoa v))
        ((= (type v) 'STR) v)
        ((= (type v) 'ENAME) (cdr (assoc 5 (entget v))))
        (t "?")))
(setq zv-codes '(1 2 3 7 10 11 12 13 14 15 40 41 42 43 44 45 50 51 62 66 67 70 71 72 73 74 410))
(defun zv-line (d / s)
  (setq s (strcat (cdr (assoc 5 d)) "\t" (cdr (assoc 0 d)) "\t" (cdr (assoc 8 d))))
  (foreach pair d
    (if (member (car pair) zv-codes)
      (setq s (strcat s "\t" (itoa (car pair)) "=" (zv-text (car pair) (cdr pair))))))
  s)
(defun zv-dump (path / f e)
  (setq f (open path "w"))
  (write-line (strcat "PDMODE=" (itoa (getvar "PDMODE")) "\tPDSIZE=" (zv-num (getvar "PDSIZE"))) f)
  (setq e (entnext))
  (while e
    (write-line (zv-line (entget e)) f)
    (setq e (entnext e)))
  (close f)
  (princ (strcat "\nzv-dump: " path))
  (princ))
