#import "@preview/charged-ieee:0.1.4": ieee
#import "@preview/cetz:0.5.3"

#set page(footer: context align(center, text(size: 8pt, fill: rgb("555555"))[#counter(page).display()]))

#show: ieee.with(
  title: [Fast LSH-256 and LSH-512 in Pure Rust on ARM and x86],
  authors: (
    (
      name: "Gustavo Wagon Widman",
      department: [Engenharia da Computação],
      organization: [Instituto de Tecnologia e Liderança (Inteli)],
      location: [São Paulo, Brazil],
      email: "gustavo.widman@sou.inteli.edu.br",
    ),
    (
      name: "João Victor Wandermurem",
      department: [Engenharia da Computação],
      organization: [Instituto de Tecnologia e Liderança (Inteli)],
      location: [São Paulo, Brazil],
      email: "joao.oliveira@sou.inteli.edu.br",
    ),
    (
      name: "Bryan Kano Ferreira",
      department: [Advisor],
      organization: [Instituto de Tecnologia e Liderança (Inteli)],
      location: [São Paulo, Brazil],
      email: "bryan.ferreira@prof.inteli.edu.br",
    ),
  ),
  index-terms: ([LSH], [KS X 3262], [hash functions], [SIMD], [Rust]),
  bibliography: bibliography("refs.bib", style: "ieee"),
)

#set text(font: "TeX Gyre Termes", lang: "en")
#show raw: set text(font: "TeX Gyre Cursor")
// Keeps C++ and Crypto++ from breaking between their plus signs.
#show regex("\\w+\\+\\+"): box
#show table.cell: set text(size: 8pt)
#set table(inset: (x: 3pt, y: 2pt))
#set math.equation(numbering: none)
#show figure.where(kind: table): set figure.caption(position: top)

#include "sections/intro.typ"
#include "sections/lsh.typ"
#include "sections/related.typ"
