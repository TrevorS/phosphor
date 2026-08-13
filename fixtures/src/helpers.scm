;; fixtures/src/helpers.scm — Steel/Scheme fixture (T037's first-class
;; twelve; tree-sitter-scheme, T083's grammar-ABI check). Standalone —
;; nothing here is loaded by the real runtime/ tree.

(define (backoff base tries)
  (min (* base (expt 2 tries)) 2000))

(define (jitter delay seed)
  (define next (bitwise-xor seed (arithmetic-shift seed -7)))
  (+ delay (modulo next 25)))

(define (retry-plan max-attempts base-delay)
  (map (lambda (tries) (backoff base-delay tries))
       (range 0 max-attempts)))
