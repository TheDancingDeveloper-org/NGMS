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
