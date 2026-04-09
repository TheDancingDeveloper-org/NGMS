const shimmerBg = 'linear-gradient(90deg, #1e293b 25%, #334155 50%, #1e293b 75%)'
const shimmerSize = '200% 100%'
const shimmerAnim = 'shimmer 1.5s infinite'

function ShimmerStyle() {
  return <style>{`@keyframes shimmer { to { background-position: -200% 0; } }`}</style>
}

export function PosterSkeleton({ count = 12, isMobile = false }: { count?: number; isMobile?: boolean }) {
  return (
    <>
      <ShimmerStyle />
      <div style={{
        display: 'grid',
        gridTemplateColumns: isMobile
          ? 'repeat(auto-fill, minmax(110px, 1fr))'
          : 'repeat(auto-fill, minmax(160px, 1fr))',
        gap: isMobile ? 10 : 16,
      }}>
        {Array.from({ length: count }).map((_, i) => (
          <div key={i} style={{
            background: '#1e293b',
            borderRadius: 12,
            overflow: 'hidden',
            border: '1px solid #334155',
          }}>
            <div style={{
              aspectRatio: '2/3',
              background: shimmerBg,
              backgroundSize: shimmerSize,
              animation: shimmerAnim,
            }} />
            <div style={{ padding: '10px 12px' }}>
              <div style={{
                height: 14, width: '70%', borderRadius: 4,
                background: shimmerBg,
                backgroundSize: shimmerSize,
                animation: shimmerAnim,
                marginBottom: 6,
              }} />
              <div style={{
                height: 12, width: '40%', borderRadius: 4,
                background: shimmerBg,
                backgroundSize: shimmerSize,
                animation: shimmerAnim,
              }} />
            </div>
          </div>
        ))}
      </div>
    </>
  )
}

export function DetailSkeleton() {
  return (
    <>
      <ShimmerStyle />
      <div>
        <div style={{
          height: 280, borderRadius: 12, marginBottom: 24,
          background: shimmerBg,
          backgroundSize: shimmerSize,
          animation: shimmerAnim,
        }} />
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          {Array.from({ length: 3 }).map((_, i) => (
            <div key={i} style={{
              height: 56, borderRadius: 10,
              background: shimmerBg,
              backgroundSize: shimmerSize,
              animation: shimmerAnim,
            }} />
          ))}
        </div>
      </div>
    </>
  )
}

export function ListSkeleton({ count = 6 }: { count?: number }) {
  return (
    <>
      <ShimmerStyle />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        {Array.from({ length: count }).map((_, i) => (
          <div key={i} style={{
            display: 'flex', alignItems: 'center', gap: 12,
            padding: '12px 14px', borderRadius: 10,
            background: '#1e293b', border: '1px solid #334155',
          }}>
            <div style={{
              width: 36, height: 54, borderRadius: 4, flexShrink: 0,
              background: shimmerBg, backgroundSize: shimmerSize, animation: shimmerAnim,
            }} />
            <div style={{ flex: 1 }}>
              <div style={{
                height: 14, width: '60%', borderRadius: 4, marginBottom: 6,
                background: shimmerBg, backgroundSize: shimmerSize, animation: shimmerAnim,
              }} />
              <div style={{
                height: 10, width: '35%', borderRadius: 4,
                background: shimmerBg, backgroundSize: shimmerSize, animation: shimmerAnim,
              }} />
            </div>
            <div style={{
              width: 70, height: 22, borderRadius: 4, flexShrink: 0,
              background: shimmerBg, backgroundSize: shimmerSize, animation: shimmerAnim,
            }} />
          </div>
        ))}
      </div>
    </>
  )
}

export function CardListSkeleton({ count = 5 }: { count?: number }) {
  return (
    <>
      <ShimmerStyle />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
        {Array.from({ length: count }).map((_, i) => (
          <div key={i} style={{
            display: 'flex', gap: 12, padding: 12,
            background: '#1e293b', borderRadius: 10, border: '1px solid #334155',
          }}>
            <div style={{
              width: 80, height: 120, borderRadius: 8, flexShrink: 0,
              background: shimmerBg, backgroundSize: shimmerSize, animation: shimmerAnim,
            }} />
            <div style={{ flex: 1 }}>
              <div style={{
                height: 16, width: '50%', borderRadius: 4, marginBottom: 8,
                background: shimmerBg, backgroundSize: shimmerSize, animation: shimmerAnim,
              }} />
              <div style={{
                height: 12, width: '30%', borderRadius: 4, marginBottom: 8,
                background: shimmerBg, backgroundSize: shimmerSize, animation: shimmerAnim,
              }} />
              <div style={{
                height: 10, width: '80%', borderRadius: 4,
                background: shimmerBg, backgroundSize: shimmerSize, animation: shimmerAnim,
              }} />
            </div>
          </div>
        ))}
      </div>
    </>
  )
}

export function RowSkeleton() {
  return (
    <>
      <ShimmerStyle />
      <div style={{ marginBottom: 32 }}>
        <div style={{ height: 20, width: 150, borderRadius: 4, marginBottom: 12, background: '#1e293b' }} />
        <div style={{ display: 'flex', gap: 16 }}>
          {Array.from({ length: 6 }).map((_, i) => (
            <div key={i} style={{
              flex: '0 0 160px', height: 280, borderRadius: 10,
              background: shimmerBg,
              backgroundSize: shimmerSize,
              animation: shimmerAnim,
            }} />
          ))}
        </div>
      </div>
    </>
  )
}
