import { createFileRoute } from '@tanstack/react-router'

import { ReviewsScreen } from '@/features/merchant/components/reviews'

export const Route = createFileRoute('/merchant/reviews')({
  component: ReviewsScreen,
})
