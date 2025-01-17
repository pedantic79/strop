# strop
Superoptimizer written in Rust

I made the decision to abandon [stoc](https://github.com/omarandlorraine/stoc)
when I realized it was simply too unwieldly to work with. I needed an excuse to
learn Rust, plus I wanted a superoptimizer that could target things other than
the 6502., So, strop was born, the *st*ochastic *op*timizer, written in *R*ust.

Okay, okay, it's not stochastic (yet). Like very early versions of stoc, it has
an exhaustive search only. Some of the warnings at build time about functions
that never get used, is because the stochastic search is not implemented yet.

### Supported architectures:
Not much here (yet). There are a few placeholders for miscellaneous
architectures, and some of the instructions have been implemented for some of
them. But I don't want to say they're *supported* as such yet. Probably the
best ones are:

- *mos6502*, because why not
- *mos65c02*, which has all the same instructions as mos6502 plus some extras
- *motorola6800*, it's related to the 6502s but has an extra register and some
  other goodies

### Theory of operation
The basic idea is to generate code better than what traditional optimising
compilers can do. A few of the reasons why that's possible:

- we can do an exhaustive search, while optimizing compilers generally do a
  greedy ascent. That means strop will find a global maximum, instead of a
  local maximum.

- we can put things like error margins, and don't-care bits on output
  variables, which can yield more opportunity for code optimization. That's
  like saying, "oh I don't care if the program computes things 100% correctly,
  so long as it's much faster", which I bet could have some utility.

- we can add different weights to each test case. That would be like saying,
  "oh, I don't care if the program is slower in the general case, so long as
  it's faster for these specific test cases."

(The last two are not implemented yet, but something I want to do eventually)

How are we going to do this? The way strop generates code is by running a code
sequence against a set of test cases (these may be generated by strop itself or
supplied by the user). The code is mutated and run against the test cases over
and over again. When the test cases all pass, we know it's a good program. As
the code is run, we can analyse it for characteristics like speed and size, and
this information can be fed into the way we mutate or select the code sequence.

### Some example runs

What if we want to multiply some number by a constant? For this example, the
number is in register B, the constant is 15, and the output is in register A.
So you'd run:

    strop --arch motorola6800 --function mult15 --search exh --live-in b --live-out a

And the program outputs:

	tba
	aba
	aba
	asla
	aba
	asla
	aba

Or let's say you want a multiply by seven routine for the 6502. So you run

    strop --arch mos6502 --function mult7 --search exh --live-in a --live-out a

Okay, the program spits out the following:

    sta 3
    asl a
    adc 3
    asl a
    adc 3

I don't yet know why location 3 was picked. And I don't know why the carry flag
wasn't cleared anywhere. That's a bug.

These programs were found by an exhaustive search. The difficulty is that this
takes a long time to run, and the runtime is only going to get worse as I add
more instructions to each architecture. Eventually the problem of long runtimes
will be mitigated by two things: miscellaneous stochastic search strategies
which can run faster by not checking Every Single Possibility, and use of
threads or something.
.
