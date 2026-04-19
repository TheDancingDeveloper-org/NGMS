import { useRef, useState, useEffect, useCallback, type ReactNode } from 'react'
import { ChevronLeft, ChevronRight, Loader2 } from 'lucide-react'

interface MediaSliderProps {
  title: string
  children: ReactNode
  isLoading?: boolean
}

export default function MediaSlider({ title, children, isLoading }: MediaSliderProps) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const [canScrollLeft, setCanScrollLeft] = useState(false)
  const [canScrollRight, setCanScrollRight] = useState(false)

  const checkScroll = useCallback(() => {
    const el = scrollRef.current
    if (!el) return
    setCanScrollLeft(el.scrollLeft > 0)
    setCanScrollRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 1)
  }, [])

  useEffect(() => {
    checkScroll()
    const el = scrollRef.current
    if (el) {
      el.addEventListener('scroll', checkScroll, { passive: true })
      const ro = new ResizeObserver(checkScroll)
      ro.observe(el)
      return () => {
        el.removeEventListener('scroll', checkScroll)
        ro.disconnect()
      }
    }
  }, [children, checkScroll])

  const scroll = useCallback((direction: 'left' | 'right') => {
    if (!scrollRef.current) return
    const amount = scrollRef.current.clientWidth * 0.8
    scrollRef.current.scrollBy({
      left: direction === 'left' ? -amount : amount,
      behavior: 'smooth',
    })
  }, [])

  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-sm font-semibold text-slate-200">{title}</h3>
        <div className="flex gap-1">
          <button
            onClick={() => scroll('left')}
            disabled={!canScrollLeft}
            className="rounded-full p-1 text-slate-400 hover:bg-slate-700 hover:text-white disabled:opacity-20 disabled:hover:bg-transparent transition-colors"
          >
            <ChevronLeft size={16} />
          </button>
          <button
            onClick={() => scroll('right')}
            disabled={!canScrollRight}
            className="rounded-full p-1 text-slate-400 hover:bg-slate-700 hover:text-white disabled:opacity-20 disabled:hover:bg-transparent transition-colors"
          >
            <ChevronRight size={16} />
          </button>
        </div>
      </div>

      {isLoading ? (
        <div className="flex h-[195px] items-center justify-center">
          <Loader2 size={20} className="animate-spin text-slate-500" />
        </div>
      ) : (
        <div
          ref={scrollRef}
          className="scrollbar-hide flex gap-2.5 overflow-x-auto"
        >
          {children}
        </div>
      )}
    </div>
  )
}
