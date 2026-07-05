var total = 0
for i in 1 to 10
    total += i
say "sum 1..10 = {total}"

repeat 3 times
    say "hip hip"

var lives = 3
while lives > 0
    say "lives: {lives}"
    lives -= 1

fn fib(n: Int) -> Int
    if n < 2
        return n
    return fib(n - 1) + fib(n - 2)

say "fib(15) = {fib(15)}"

use math
say "sqrt(2) is about {math.sqrt(2.0)}"

use random
random.seed(7)
say "dice: {random.int(1, 6)} and {random.int(1, 6)}"
