import ts from 'typescript'
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { tmpdir } from 'node:os'

const workDir = join(tmpdir(), `personal-pilot-direct-proxy-${process.pid}`)
const sourcePath = join(process.cwd(), 'src/modules/browser/directProxyImport.ts')
const bundle = join(workDir, 'directProxyImport.mjs')

const cases = [
  ['服务器:端口:账号:密码', 'proxy.example.com:30000:user:pass'],
  ['服务器:端口@账号:密码', 'proxy.example.com:30000@user:pass'],
  ['账号:密码:服务器:端口', 'user:pass:proxy.example.com:30000'],
  ['账号:密码@服务器:端口', 'user:pass@proxy.example.com:30000'],
  ['账号:数字密码:服务器:端口', 'user:12345:proxy.example.com:30000', 'http://user:12345@proxy.example.com:30000'],
  ['账号:数字密码@服务器:端口', 'user:12345@proxy.example.com:30000', 'http://user:12345@proxy.example.com:30000'],
  ['标准 URL', 'http://user:pass@proxy.example.com:30000'],
  ['标准 URL 保留 VPS 参数', 'http://user:pass@proxy.example.com:30000?pp_via_ssh=panda', 'http://user:pass@proxy.example.com:30000?pp_via_ssh=panda'],
  ['无账号', 'proxy.example.com:30000'],
]

try {
  await mkdir(workDir, { recursive: true })
  const source = await readFile(sourcePath, 'utf8')
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.ES2022,
    },
  })
  await writeFile(bundle, compiled.outputText)

  const { buildDirectImportCandidate, parseDirectProxyLine } = await import(pathToFileURL(bundle).href)
  for (const [label, value, explicitExpected] of cases) {
    const parsed = parseDirectProxyLine(value)
    if (!parsed) {
      throw new Error(`${label} 未解析`)
    }
    const candidate = buildDirectImportCandidate({
      proxyName: '',
      protocol: 'http',
      server: value,
      port: '',
      username: '',
      password: '',
    })
    const expected = explicitExpected || (label === '无账号'
      ? 'http://proxy.example.com:30000'
      : 'http://user:pass@proxy.example.com:30000')
    if (candidate.proxyConfig !== expected) {
      throw new Error(`${label} 结果错误: ${candidate.proxyConfig}`)
    }
  }

  console.log(JSON.stringify({
    ok: true,
    checked: cases.map(([label]) => label),
  }, null, 2))
} finally {
  await rm(workDir, { recursive: true, force: true })
}
