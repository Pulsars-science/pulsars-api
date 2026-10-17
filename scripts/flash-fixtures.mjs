#!/usr/bin/env node
// Writes, from the TypeScript port of the flash questions
// (pulsars-game/src/flash/):
//
// - tests/fixtures/flash/duels.json: the "défi" algorithm of
//   questionsFlash.ts, run on every subject and on all generators, in
//   French, English and Russian;
// - tests/fixtures/flash/generators-ru.json: every generator's walls
//   (`serie()`) in Russian, which the site's PHP fixtures do not cover.
//
// Nothing is installed: Node (>= 23, type stripping) loads the game's
// TypeScript sources through the small loader below.
//
// Usage: node scripts/flash-fixtures.mjs ../pulsars-game

import { writeFileSync } from 'node:fs'
import { register } from 'node:module'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const game = resolve(process.argv[2] ?? '../pulsars-game')
const src = pathToFileURL(`${game}/src/`).href

// Resolves the `@/` alias and extensionless imports, and loads JSON as a
// default export, as Vite does.
const hooks = `
import { existsSync } from 'node:fs'
const src = ${JSON.stringify(src)}
export async function resolve(specifier, context, next) {
  if (specifier.startsWith('@/')) specifier = src + specifier.slice(2)
  if ((specifier.startsWith('.') || specifier.startsWith('file:')) && !/\\.[a-z]+$/.test(specifier)) {
    const url = new URL(specifier, context.parentURL)
    specifier = existsSync(new URL(url.href + '.ts')) ? url.href + '.ts' : url.href + '/index.ts'
  }
  return next(specifier, context)
}
export async function load(url, context, next) {
  if (url.endsWith('.json')) {
    const { readFileSync } = await import('node:fs')
    const text = readFileSync(new URL(url), 'utf8')
    return { format: 'module', source: 'export default ' + text, shortCircuit: true }
  }
  return next(url, context)
}
`
register(`data:text/javascript,${encodeURIComponent(hooks)}`)

const { GENERATEURS } = await import(`${src}flash/catalogue.ts`)
const { Hasard } = await import(`${src}flash/hasard.ts`)
const { changerLangue } = await import(`${src}i18n/index.ts`)
const { serie } = await import(`${src}flash/questionsFlash.ts`)

// questionsFlash.ts, `choix` and `silhouette` (not exported).
const CHOIX_PAR_QUESTION = 4
const ESSAIS_LEURRE = 60
function silhouette(reponse) {
  return reponse.replace(/[\p{Nd}\s.,\-−⁰¹²³⁴⁵⁶⁷⁸⁹⁻]/gu, '')
}
function choix(generateur, hasard, reponse) {
  const liste = [reponse]
  const forme = silhouette(reponse)
  for (const memeForme of [true, false]) {
    for (let essai = 0; essai < ESSAIS_LEURRE && liste.length < CHOIX_PAR_QUESTION; essai++) {
      const leurre = generateur.tirer(hasard).reponse
      if (liste.includes(leurre)) continue
      if (memeForme && silhouette(leurre) !== forme) continue
      liste.push(leurre)
    }
  }
  return hasard.melanger(liste)
}

// `defi()` with the candidates given instead of the maths generators.
function defi(candidats, graine, nombre) {
  const hasard = new Hasard(graine)
  const questions = []
  for (let rang = 1; rang <= nombre; rang++) {
    const generateur = candidats[hasard.entier(0, candidats.length - 1)]
    const carte = generateur.tirer(hasard)
    questions.push({
      question: carte.question,
      reponse: carte.reponse,
      choix: choix(generateur, hasard, carte.reponse),
      serie: generateur.titre(),
    })
  }
  return questions
}

changerLangue('fr')
const SUBJECTS = { Mathématiques: 'maths', Physique: 'physics', Chimie: 'chemistry', Informatique: 'computer-science' }
const subjectOf = new Map(GENERATEURS.map((g) => [g, SUBJECTS[g.matiere()]]))

const duels = []
for (const subject of [null, ...Object.values(SUBJECTS)]) {
  const candidats = GENERATEURS.filter((g) => subject === null || subjectOf.get(g) === subject)
  for (const graine of [1, 42, 31337, 999999]) {
    const duel = { subject, graine }
    for (const langue of ['fr', 'en', 'ru']) {
      changerLangue(langue)
      duel[langue] = defi(candidats, graine, 60)
    }
    duels.push(duel)
  }
}

const out = new URL('../tests/fixtures/flash/duels.json', import.meta.url)
writeFileSync(out, JSON.stringify(duels) + '\n')
console.log(`${duels.length} duels written to ${out.pathname}`)

// The walls of every generator in Russian, as the PHP fixtures give them in
// French and English (same seeds and card count).
changerLangue('ru')
const walls = GENERATEURS.map((g) => ({
  cle: g.cle(),
  titre: g.titre(),
  titreCourt: g.titreCourt(),
  description: g.description(),
  matiere: g.matiere(),
  niveau: g.niveau(),
  series: [1, 7, 42, 2024, 31337, 123456, 777777, 999999].map((graine) => ({ graine, cartes: serie(g, graine, 40) })),
}))
const outRu = new URL('../tests/fixtures/flash/generators-ru.json', import.meta.url)
writeFileSync(outRu, JSON.stringify(walls) + '\n')
console.log(`${walls.length} generators written to ${outRu.pathname}`)
