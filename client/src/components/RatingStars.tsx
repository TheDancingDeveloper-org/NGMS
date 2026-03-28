import { useState } from 'react'
import { Star } from 'lucide-react'
import { useRating, useSetRating, useDeleteRating } from '../hooks/useApi'

interface Props {
  mediaType: 'series' | 'movie'
  mediaId: number
}

export default function RatingStars({ mediaType, mediaId }: Props) {
  const { data: ratingInfo, isLoading } = useRating(mediaType, mediaId)
  const setRatingMut = useSetRating()
  const deleteRatingMut = useDeleteRating()
  const [hoverStar, setHoverStar] = useState(0)

  const userRating = ratingInfo?.userRating ?? null
  const averageRating = ratingInfo?.averageRating ?? 0
  const ratingCount = ratingInfo?.ratingCount ?? 0

  // Convert 1-10 scale to 1-5 stars for display
  // Stars 1-5 map to ratings 2,4,6,8,10
  const starValue = userRating ? Math.round(userRating / 2) : 0
  const displayStar = hoverStar || starValue

  const handleClick = (star: number) => {
    const rating = star * 2 // Convert star (1-5) to rating (2-10)
    if (userRating === rating) {
      // Click same rating to remove
      deleteRatingMut.mutate({ mediaType, mediaId })
    } else {
      setRatingMut.mutate({ mediaType, mediaId, rating })
    }
  }

  if (isLoading) return null

  const avgStars = averageRating / 2

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
      <div
        style={{ display: 'flex', alignItems: 'center', gap: 2 }}
        onMouseLeave={() => setHoverStar(0)}
      >
        {[1, 2, 3, 4, 5].map((star) => (
          <button
            key={star}
            onClick={() => handleClick(star)}
            onMouseEnter={() => setHoverStar(star)}
            style={{
              background: 'none',
              border: 'none',
              cursor: 'pointer',
              padding: 2,
              display: 'flex',
            }}
          >
            <Star
              size={20}
              fill={star <= displayStar ? '#eab308' : 'none'}
              color={star <= displayStar ? '#eab308' : '#64748b'}
            />
          </button>
        ))}
        {userRating != null && (
          <span style={{ fontSize: 12, color: '#94a3b8', marginLeft: 4 }}>
            {userRating}/10
          </span>
        )}
      </div>
      {ratingCount > 0 && (
        <span style={{ fontSize: 11, color: '#64748b' }}>
          Avg: {avgStars.toFixed(1)}/5 ({ratingCount} {ratingCount === 1 ? 'rating' : 'ratings'})
        </span>
      )}
    </div>
  )
}
