import { useState, useEffect } from 'react'
import { Star } from 'lucide-react'
import { api } from '../api'

interface Props {
  mediaType: 'series' | 'movie'
  mediaId: number
}

export default function RatingStars({ mediaType, mediaId }: Props) {
  const [userRating, setUserRating] = useState<number | null>(null)
  const [averageRating, setAverageRating] = useState(0)
  const [ratingCount, setRatingCount] = useState(0)
  const [hoverStar, setHoverStar] = useState(0)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    api
      .getRating(mediaType, mediaId)
      .then((info) => {
        setUserRating(info.userRating)
        setAverageRating(info.averageRating)
        setRatingCount(info.ratingCount)
      })
      .catch(() => {})
      .finally(() => setLoading(false))
  }, [mediaType, mediaId])

  // Convert 1-10 scale to 1-5 stars for display
  // Stars 1-5 map to ratings 2,4,6,8,10
  const starValue = userRating ? Math.round(userRating / 2) : 0
  const displayStar = hoverStar || starValue

  const handleClick = async (star: number) => {
    const rating = star * 2 // Convert star (1-5) to rating (2-10)
    try {
      if (userRating === rating) {
        // Click same rating to remove
        await api.deleteRating(mediaType, mediaId)
        setUserRating(null)
        // Refresh average
        const info = await api.getRating(mediaType, mediaId)
        setAverageRating(info.averageRating)
        setRatingCount(info.ratingCount)
      } else {
        const result = await api.setRating(mediaType, mediaId, rating)
        setUserRating(result.rating)
        // Refresh average
        const info = await api.getRating(mediaType, mediaId)
        setAverageRating(info.averageRating)
        setRatingCount(info.ratingCount)
      }
    } catch (e) {
      console.error('Rating failed:', e)
    }
  }

  if (loading) return null

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
