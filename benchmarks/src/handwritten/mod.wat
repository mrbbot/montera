(module
  (func $fib (export "fib") (param i32) (result i32)
    ;; if (n <= 1)
    local.get 0
   	i32.const 1
    i32.le_s
    (if (result i32)
      (then
        ;; return 1
        i32.const 1
      )
      (else
        ;; fib(n - 1)
        local.get 0
        i32.const 1
        i32.sub
        call $fib

        ;; fib(n - 2)
        local.get 0
        i32.const 2
        i32.sub
        call $fib

        ;; return fib(n - 1) + fib(n - 2)
        i32.add
      )
    )
  )
  (func (export "gcd") (param i32 i32) (result i32)
    (local $tmp i32)
    (loop $loop
      local.get 1
      i32.const 0
      i32.ne
      (if
      	(then
          ;; tmp = b
          local.get 1
          local.set $tmp

          ;; b = a % b
          local.get 0
          local.get 1
          i32.rem_s
          local.set 1

          ;; a = tmp
          local.get $tmp
          local.set 0

          ;; continue
          br $loop
        )
      )
    )
    local.get 0
  )
  (memory 1)
  (global $next (mut i32) i32.const 8)
  (func (export "sum") (param $a i32) (param $b i32) (param $c i32) (result i32)
    (local $triple i32)
    ;; Allocate 12 bytes for storing triple of integers
    global.get $next
    local.set $triple
    global.get $next
    i32.const 12
    i32.add
    global.set $next
    ;; Store 3 integers in memory
    local.get $triple
    local.get $a
    i32.store offset=0
    local.get $triple
    local.get $b
    i32.store offset=4
    local.get $triple
    local.get $c
    i32.store offset=8
    ;; Load and sum 3 integers from memory
    local.get $triple
    i32.load offset=0
    local.get $triple
    i32.load offset=4
    i32.add
    local.get $triple
    i32.load offset=8
    i32.add
  )
)
