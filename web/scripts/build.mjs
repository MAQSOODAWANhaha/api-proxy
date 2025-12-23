import * as esbuild from 'esbuild'
import { rimraf } from 'rimraf'
import stylePlugin from 'esbuild-style-plugin'
import autoprefixer from 'autoprefixer'
import tailwindcss from 'tailwindcss'
import { readFileSync, writeFileSync } from 'fs'
import { join } from 'path'

const args = process.argv.slice(2)
const isProd = args[0] === '--production'

await rimraf('dist')

// 自定义HTML处理插件
const htmlProcessPlugin = {
  name: 'html-process',
  setup(build) {
    if (isProd) {
      build.onEnd(() => {
        const htmlPath = join('dist', 'index.html')
        try {
          let htmlContent = readFileSync(htmlPath, 'utf8')
          // 移除热重载代码
          htmlContent = htmlContent.replace(
            /\s*<script>\s*new EventSource\('\/esbuild'\)\.addEventListener\('change',[\s\S]*?<\/script>\s*/,
            ''
          )
          writeFileSync(htmlPath, htmlContent)
          console.log('✓ 已从生产构建中移除热重载代码')
        } catch (error) {
          console.warn('处理HTML文件时出错:', error)
        }
      })
    }
  }
}

/**
 * @type {esbuild.BuildOptions}
 */
const esbuildOpts = {
  color: true,
  entryPoints: ['src/main.tsx', 'index.html'],
  outdir: 'dist',
  entryNames: '[name]',
  write: true,
  bundle: true,
  format: 'iife',
  sourcemap: isProd ? false : 'linked',
  minify: isProd,
  treeShaking: true,
  jsx: 'automatic',
  define: {
    __DEV__: JSON.stringify(!isProd),
  },
  loader: {
    '.html': 'copy',
    '.png': 'file',
  },
  plugins: [
    stylePlugin({
      postcss: {
        plugins: [tailwindcss, autoprefixer],
      },
    }),
    htmlProcessPlugin,
  ],
}

if (isProd) {
  await esbuild.build(esbuildOpts)
} else {
  const ctx = await esbuild.context(esbuildOpts)
  await ctx.watch()
  const { hosts, port } = await ctx.serve()
  console.log(`Running on:`)
  hosts.forEach((host) => {
    console.log(`http://${host}:${port}`)
  })
}
