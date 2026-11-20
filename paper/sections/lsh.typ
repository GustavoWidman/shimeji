#import "@preview/cetz:0.5.3"

= The LSH family <lsh>

LSH-256 works on 32-bit words and returns digests of 224 or 256 bits @kim2014lsh. LSH-512 works on 64-bit words and returns 224, 256, 384 or 512 bits. Both are wide-pipe Merkle–Damgård constructions @lucks2005failure. A chaining variable of sixteen words, twice the size of the longest digest, absorbs the message one 32-word block at a time. Padding appends a one bit and then zero bits up to the end of the block. It adds no length field, so a message that fills its last block gains a whole block of padding. The digest is the XOR of the two halves of the final chaining variable, truncated to the digest length. @tab:params lists the parameters that differ between the two.

#figure(
  table(
    columns: (auto, auto, auto),
    align: (left, left, left),
    stroke: none,
    table.hline(),
    table.header([], [LSH-256], [LSH-512]),
    table.hline(stroke: 0.5pt),
    [Word size $w$], [32 bits], [64 bits],
    [Block], [128 bytes], [256 bytes],
    [Steps $N_s$], [26], [28],
    [$(alpha, beta)$, even steps], [(29, 1)], [(23, 59)],
    [$(alpha, beta)$, odd steps], [(5, 17)], [(7, 3)],
    [$gamma_0, ..., gamma_7$], [0, 8, 16, 24, 24, 16, 8, 0], [0, 16, 32, 48, 8, 24, 40, 56],
    [Digest bits], [224, 256], [224, 256, 384, 512],
    table.hline(),
  ),
  caption: [Parameters of LSH-256 and LSH-512.],
) <tab:params>

The compression function reads a block as two sub-messages $M_0$ and $M_1$ of sixteen words and expands them into $N_s + 1$ sub-messages,
$ M_j [l] = M_(j-1) [l] ⊞ M_(j-2) [tau(l)], quad 2 <= j <= N_s, $
where $⊞$ is addition modulo $2^w$ and $tau$ permutes the words within each half of a sub-message. A state $T$ starts as the chaining variable and runs through $N_s$ steps. Step $j$ XORs $M_j$ into $T$, applies the mix function of @fig:mix to each of the eight column pairs $(T[l], T[l+8])$, and permutes the sixteen words by a fixed permutation $sigma$. The new chaining variable is $T ⊕ M_(N_s)$.

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

The mix function adds, rotates and XORs. Its rotation amounts $alpha_j$ and $beta_j$ alternate between even and odd steps, and its last rotation $gamma_l$ depends only on the column. Every $gamma_l$ is a multiple of eight, so that rotation moves whole bytes. Each step XORs in its own vector of eight step constants $"SC"_j$. Each vector is the one before it plus itself rotated left by eight bits, so all of them follow from $"SC"_0$.
