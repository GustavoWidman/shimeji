#import "@preview/cetz:0.5.3"

= The LSH family <lsh>

In this section we describe how LSH works, following the specification @kim2014lsh. We describe LSH-256 in detail and point out what changes in LSH-512.

== Overview

LSH works on words: 32-bit words in LSH-256 and 64-bit words in LSH-512. LSH-256 produces digests of 224 or 256 bits, and LSH-512 produces digests of 224, 256, 384 or 512 bits @kim2014lsh. Both use a wide-pipe Merkle–Damgård construction @lucks2005failure. This means that the hash keeps an internal state, called the chaining variable, that is larger than the output. In LSH the chaining variable has sixteen words, which is 64 bytes in LSH-256 and 128 bytes in LSH-512, twice the size of the longest digest. The message is cut into blocks of 32 words (128 bytes in LSH-256 and 256 bytes in LSH-512), and each block is mixed into the chaining variable by the compression function. @tab:params shows the parameters that are different in the two versions.

#figure(
  table(columns: (auto, auto, auto), inset: (x: 2pt, y: 2pt), align: left, stroke: none,
    table.hline(), table.header([], [LSH-256], [LSH-512]), table.hline(stroke: 0.5pt),
    [Word size $w$], [32 bits], [64 bits],
    [Chaining variable], [64 bytes], [128 bytes],
    [Block], [128 bytes], [256 bytes],
    [Steps $N_s$], [26], [28],
    [$(alpha, beta)$, even steps], [(29, 1)], [(23, 59)],
    [$(alpha, beta)$, odd steps], [(5, 17)], [(7, 3)],
    [$gamma_0, ..., gamma_7$], [0, 8, 16, 24, 24, 16, 8, 0], [0, 16, 32, 48, 8, 24, 40, 56],
    [Digest bits], [224, 256], [224, 256, 384, 512], table.hline()),
  caption: [Parameters of LSH-256 and LSH-512.],
) <tab:params>

== Padding

Before hashing, the message is padded: a single 1 bit is added at the end, followed by 0 bits until the block is complete. Since messages are usually whole bytes, this is the same as adding the byte 0x80 and then zero bytes. For example, the two-byte message "oi" (0x6F 0x69) becomes one 128-byte LSH-256 block that starts with 0x6F 0x69 0x80 and has only zeros after that. Unlike SHA-2, LSH does not add the length of the message to the padding. Because of this, a message that already fills its last block exactly still needs one more block, made only of padding. The bytes of each block are read as words in little-endian order.

== Initialization

The chaining variable starts with a fixed initialization vector (IV) that is different for each digest size. The IV values are given as tables in the specification and in the reference code @kim2014lsh, @kisa2026lsh, so they are public and the same for everyone. They are not arbitrary numbers, though. Looking at the reference code @dai2026cryptopp, the IV can be recomputed by starting from a chaining variable that has the maximum digest length in bytes in word 0 (32 for LSH-256, 64 for LSH-512), the output length in bits in word 1, and zeros everywhere else, and then running the steps of @steps without any message. For LSH-256-256, for example, the first IV word obtained this way is 0x46A10F1F, which matches the published table.

== Message expansion

The compression function receives the current chaining variable and one block. It first reads the block as two sub-messages, $M_0$ (words 0 to 15) and $M_1$ (words 16 to 31), and then uses them to compute $N_s + 1$ sub-messages in total, one for each step and one more for the end:

#math.equation(block: true, numbering: "(1)")[$ M_j[l] = M_(j-1)[l] ⊞ M_(j-2)[tau(l)] $]

for $2 <= j <= N_s$ and $0 <= l < 16$,

where $⊞$ is addition modulo $2^w$. In other words, word $l$ of the new sub-message is the sum of word $l$ of the previous sub-message and of a word of the sub-message before that, chosen by the permutation $tau$.

The permutation $tau$ is not calculated; it is a fixed table defined in the specification @kim2014lsh, shown in @tab:tau. It works inside each half of a sub-message: for words 0 to 7 it is used as written, and for words 8 to 15 the same table is used with 8 added to every index. LSH-256 and LSH-512 use the same table.

#figure({
  set text(size: 7pt)
  show table.cell: set text(size: 7pt)
  table(columns: 17, inset: (x: 1.5pt, y: 2pt), align: center, stroke: none,
    table.hline(), [$l$], [0], [1], [2], [3], [4], [5], [6], [7], [8], [9], [10], [11], [12], [13], [14], [15], table.hline(stroke: 0.5pt),
    [$tau(l)$], [3], [2], [0], [1], [7], [4], [5], [6], [11], [10], [8], [9], [15], [12], [13], [14], table.hline())
  }, caption: [Message permutation $tau$ (word $l$ of $M_j$ uses word $tau(l)$ of $M_(j-2)$).],
) <tab:tau>

There are a few reasons why it makes sense for $tau$ to be a table. First, since it is public and fixed, every implementation produces exactly the same sub-messages. Second, the indices are known when the code is compiled, so applying $tau$ is just reading words from fixed positions and does not cost any computation at run time. Third, the table has a simple structure: in each half it forms two cycles of four words. Following where the content of word 0 goes, it moves to position 2, then to position 1, then to position 3, and then back to position 0, and words 4 to 7 go around in the same way (4, 5, 6, 7). This also means that the expansion by itself never mixes a word of one cycle with a word of the other cycle; this mixing happens later, in the state, through the mix function and the permutation $sigma$. Finally, the expansion depends only on the block and not on the chaining variable, so all the sub-messages can be computed before or in parallel with the steps. Our hybrid ARM path uses this property, and in our batched path, where each register holds the same word of several messages, applying $tau$ is only a matter of reading a different register.

== Steps <steps>

Next, a state T is set to the current chaining variable and goes through $N_s$ steps (26 for LSH-256 and 28 for LSH-512). Each step $j$ has three parts:
+ Message addition: the sub-message $M_j$ is XORed into the state, $T[l] arrow.l T[l] ⊕ M_j[l]$ for $0 <= l < 16$.
+ Mix: the sixteen words are seen as eight column pairs $(T[l], T[l+8])$, $0 <= l < 8$, and each pair goes through the mix function of @fig:mix. Writing $X = T[l]$ and $Y = T[l+8]$, the mix function computes, in this order:

$ X arrow.l X ⊞ Y, quad X arrow.l (X <<< alpha_j) ⊕ "SC"_j[l], $

$ Y arrow.l X ⊞ Y, quad Y arrow.l Y <<< beta_j, $

$ X arrow.l X ⊞ Y, quad Y arrow.l Y <<< gamma_l, $

where $<<<$ is rotation to the left. Every operation uses the result of the one before, so inside one pair the mix function is a serial chain. The eight pairs, however, are independent of each other.
#enum(start: 3)[Word permutation: the sixteen words are moved to new positions by the fixed permutation $sigma$ in @tab:sigma, so that in the next step each column pair combines words that were in different pairs before. Like $tau$, $sigma$ is the same for LSH-256 and LSH-512.]

#figure({
  set text(size: 7pt)
  show table.cell: set text(size: 7pt)
  table(columns: 17, inset: (x: 1.5pt, y: 2pt), align: center, stroke: none,
    table.hline(), [$i$], [0], [1], [2], [3], [4], [5], [6], [7], [8], [9], [10], [11], [12], [13], [14], [15], table.hline(stroke: 0.5pt),
    [$s(i)$], [6], [4], [5], [7], [12], [15], [14], [13], [2], [0], [1], [3], [8], [11], [10], [9], table.hline())
  }, caption: [Word permutation $sigma$. Position $i$ receives word $s(i)$.],
) <tab:sigma>

#figure(
  cetz.canvas(length: 0.42cm, {
    import cetz.draw: *
    let (x, y) = (0, 4)
    let node(p, s) = {
      circle(p, radius: 0.36, fill: white, stroke: none)
      content(p, text(size: 13pt, s))
    }
    let rot(p, s) = {
      rect((p.at(0) - 1.4, p.at(1) - 0.42), (p.at(0) + 1.4, p.at(1) + 0.42), fill: white, stroke: 0.5pt)
      content(p, s)
    }
    let tap(from, to) = {
      circle(from, radius: 0.09, fill: black)
      line(from, to, mark: (end: ">", fill: black, scale: 0.6))
    }
    let feed(from, to, label, anchor) = {
      line(from, to, mark: (end: ">", fill: black, scale: 0.6))
      content(from, label, anchor: anchor, padding: 0.15)
    }
    line((x, 0), (x, -9.5), mark: (end: ">", fill: black, scale: 0.6))
    line((y, 0), (y, -9.5), mark: (end: ">", fill: black, scale: 0.6))
    content((x, 0), $T[l]$, anchor: "south", padding: 0.15)
    content((y, 0), $T[l+8]$, anchor: "south", padding: 0.15)

    feed((x - 2, -0.9), (x - 0.36, -0.9), $M_j [l]$, "east")
    node((x, -0.9), [$⊕$])
    feed((y + 2, -0.9), (y + 0.36, -0.9), $M_j [l+8]$, "west")
    node((y, -0.9), [$⊕$])

    tap((y, -2), (x + 0.36, -2))
    node((x, -2), [$⊞$])
    rot((x, -3), $<<< alpha_j$)
    feed((x - 2, -4), (x - 0.36, -4), $"SC"_j [l]$, "east")
    node((x, -4), [$⊕$])

    tap((x, -5.1), (y - 0.36, -5.1))
    node((y, -5.1), [$⊞$])
    rot((y, -6.1), $<<< beta_j$)

    tap((y, -7.2), (x + 0.36, -7.2))
    node((x, -7.2), [$⊞$])
    rot((y, -8.2), $<<< gamma_l$)

    content((x, -9.5), $T'[l]$, anchor: "north", padding: 0.15)
    content((y, -9.5), $T'[l+8]$, anchor: "north", padding: 0.15)
  }),
  caption: [One step of LSH on the column pair $(T[l], T[l+8])$, $0 <= l < 8$: message addition and the mix function. $⊕$ is XOR, $⊞$ is addition modulo $2^w$ and $<<<$ is rotation to the left. After all eight pairs, the step permutes the sixteen words by $sigma$.],
) <fig:mix>

The rotation amounts change during the hash. $alpha_j$ and $beta_j$ depend on whether the step number is even or odd (Table I), and $gamma_l$ depends only on the column. All the values of $gamma_l$ are multiples of eight, so the last rotation moves whole bytes and can be done with a byte shuffle. Each step also uses its own vector $"SC"_j$ of eight step constants. Only $"SC"_0$ is given directly; every other vector is computed from the previous one as

#math.equation(block: true, numbering: "(1)")[$ "SC"_j[l] = "SC"_(j-1)[l] ⊞ ("SC"_(j-1)[l] <<< 8). $]

The constants make each step different from the others. Without them, for example, a state with all words equal to zero would stay zero after the mix function, because adding and rotating zeros always gives zero.

== Finalization

After the last step, the last sub-message is XORed into the state, and $T ⊕ M_(N_s)$ becomes the new chaining variable. The next block (if there is one) starts from this value instead of the IV. When all the blocks have been processed, the digest is computed by XORing the two halves of the chaining variable, word $l$ with word $l+8$ for $0 <= l < 8$, and truncating the result to the digest length. In LSH-256, for example, this gives 8 words, which is 32 bytes or 256 bits.
