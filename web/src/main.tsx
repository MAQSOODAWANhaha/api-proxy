import { createRoot } from 'react-dom/client'
import './shadcn.css'
import App from './App'
import faviconUrl from './assets/favicon.png'

const ensureFavicon = (href: string) => {
  const existing =
    document.querySelector<HTMLLinkElement>("link[rel~='icon']") ??
    (() => {
      const link = document.createElement('link')
      link.rel = 'icon'
      document.head.appendChild(link)
      return link
    })()

  if (existing.href !== href) {
    existing.type = 'image/png'
    existing.sizes = '96x96'
    existing.href = href
  }
}

ensureFavicon(faviconUrl)

const root = createRoot(document.getElementById('app')!)
root.render(<App />)
