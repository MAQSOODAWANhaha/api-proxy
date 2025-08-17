/**
 * SearchInput - topbar search box with subtle styling.
 */

import { Search } from 'lucide-react'
import { Input } from '@/components/ui/input'
import { useState } from 'react'

interface Props {
  placeholder?: string
  onSearch?: (value: string) => void
}

export default function SearchInput({ placeholder = 'Search here...', onSearch }: Props) {
  const [value, setValue] = useState('')
  return (
    <div className="relative w-full max-w-xl">
      <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground size-4" />
      <Input
        value={value}
        onChange={(e) => {
          const v = e.target.value
          setValue(v)
          onSearch?.(v)
        }}
        placeholder={placeholder}
        className="pl-9 h-10 bg-muted/40 dark:bg-muted/20 border-0 focus-visible:ring-2 focus-visible:ring-indigo-500"
      />
    </div>
  )
}