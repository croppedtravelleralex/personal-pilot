import { existsSync, copyFileSync, mkdirSync, readdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const legacyScript = join(here, '..', 'frontend', 'scripts', 'ensure-rollup-native.mjs')

if (existsSync(legacyScript)) {
  await import(pathToFileURL(legacyScript).href)
}

const publicDir = join(here, '..', 'public')
mkdirSync(publicDir, { recursive: true })

const sourceCandidates = [
  join(here, '..', 'frontend', 'public', 'favicon.png'),
  join(here, '..', 'frontend', 'src', 'resources', 'images', 'logo.png'),
  join(here, '..', 'build', 'appicon.png'),
]

const faviconPath = join(publicDir, 'favicon.png')
if (!existsSync(faviconPath)) {
  const source = sourceCandidates.find((candidate) => existsSync(candidate))
  if (source) {
    copyFileSync(source, faviconPath)
  }
}

try {
  readdirSync(publicDir)
} catch {
  // Keep postinstall non-fatal; Vite will report missing assets if they matter.
}
