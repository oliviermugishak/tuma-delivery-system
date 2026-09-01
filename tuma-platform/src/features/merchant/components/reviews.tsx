/**
 * Merchant — Reviews (coverage §7.16): KPIs (avg rating, this month),
 * list Order · Customer · Rating · Comment · Reply status; reply =
 * inline composer in a drawer; filter by rating; designed empty state.
 *
 * BACKEND GAP (G7): no reviews endpoints — GET /v1/merchant/reviews
 * (+ POST .../reviews/{id}/reply). Demo seed rows carry the flow.
 */
import { useMemo, useState } from 'react'
import { toast } from 'sonner'

import {
  Button,
  Chip,
  CopyableId,
  DataTable,
  Drawer,
  DrawerSection,
  EmptyState,
  Icon,
  KpiCard,
  KpiGrid,
  PageHead,
  Status,
  Textarea,
  Toolbar,
  type Column,
} from '@/components/ds'
import { demoReviews, type DemoReview } from '@/features/demo/seed'
import { dateTime, num } from '@/lib/format'

function Stars({ rating }: { rating: number }) {
  return (
    <span className="inline-flex items-center gap-0.5" aria-label={`${rating} of 5 stars`}>
      {Array.from({ length: 5 }).map((_, i) => (
        <Icon
          key={i}
          name="star"
          label=""
          size={16}
          filled={i < rating}
          className={i < rating ? 'text-brand' : 'text-text3'}
        />
      ))}
    </span>
  )
}

export function ReviewsScreen() {
  const [reviews, setReviews] = useState<DemoReview[]>(demoReviews)
  const [ratingFilter, setRatingFilter] = useState<number | null>(null)
  const [replyTarget, setReplyTarget] = useState<DemoReview | null>(null)
  const [reply, setReply] = useState('')

  const filtered = useMemo(
    () =>
      reviews.filter(
        (r) => ratingFilter == null || r.rating === ratingFilter,
      ),
    [reviews, ratingFilter],
  )

  const avg =
    reviews.length > 0
      ? Math.round((reviews.reduce((s, r) => s + r.rating, 0) / reviews.length) * 10) / 10
      : null
  const thisMonth = reviews.length
  const unanswered = reviews.filter((r) => r.reply == null).length

  const columns: Column<DemoReview>[] = [
    { key: 'order', header: 'Order', cell: (r) => <CopyableId id={r.orderNumber} /> },
    { key: 'customer', header: 'Customer', cell: (r) => r.customer },
    { key: 'rating', header: 'Rating', cell: (r) => <Stars rating={r.rating} /> },
    {
      key: 'comment',
      header: 'Comment',
      cell: (r) => (
        <span className="block max-w-100 truncate text-text2" title={r.comment}>
          {r.comment}
        </span>
      ),
    },
    {
      key: 'reply',
      header: 'Reply',
      cell: (r) =>
        r.reply ? (
          <Status tone="success" small>
            Replied
          </Status>
        ) : (
          <Status tone="warning" small>
            Not replied
          </Status>
        ),
    },
    {
      key: 'at',
      header: 'Received',
      cell: (r) => <span className="text-text2">{dateTime(r.at)}</span>,
    },
  ]

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Reviews"
        sub="What customers say after their orders — replies are public, word for word"
      />

      {reviews.length === 0 ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <EmptyState
            icon="reviews"
            title="No reviews yet"
            hint="Reviews arrive after deliveries — the first one will land here."
          />
        </div>
      ) : (
        <>
          <KpiGrid>
            <KpiCard
              label="Average rating"
              value={avg != null ? String(avg) : 'No data'}
              unit={avg != null ? '/ 5' : undefined}
            />
            <KpiCard label="This month" value={num(thisMonth)} vs="reviews received" />
            <KpiCard
              label="Not replied"
              value={num(unanswered)}
              vs="replies are public"
            />
            <KpiCard
              label="Five stars"
              value={num(reviews.filter((r) => r.rating === 5).length)}
              vs="worth celebrating"
            />
          </KpiGrid>

          <Toolbar
            resultCount={`${filtered.length} reviews`}
          >
            {[1, 2, 3, 4, 5].map((n) => (
              <Chip
                key={n}
                on={ratingFilter === n}
                onClick={() => setRatingFilter(ratingFilter === n ? null : n)}
              >
                {n} star{n > 1 ? 's' : ''}
              </Chip>
            ))}
          </Toolbar>

          <DataTable
            columns={columns}
            rows={filtered}
            rowKey={(r) => r.id}
            onRowOpen={(r) => {
              setReplyTarget(r)
              setReply(r.reply ?? '')
            }}
            empty={null}
          />
        </>
      )}

      <Drawer
        open={replyTarget != null}
        onClose={() => setReplyTarget(null)}
        title={
          <>
            Review · order <CopyableId id={replyTarget?.orderNumber ?? ''} />
          </>
        }
        subtitle={replyTarget ? dateTime(replyTarget.at) : undefined}
        status={replyTarget ? <Stars rating={replyTarget.rating} /> : undefined}
        footer={
          replyTarget ? (
            <Button
              variant="primary"
              className="flex-1"
              disabled={!reply.trim()}
              onClick={() => {
                // BACKEND GAP (G7): POST /v1/merchant/reviews/{id}/reply.
                setReviews((prev) =>
                  prev.map((r) =>
                    r.id === replyTarget.id ? { ...r, reply } : r,
                  ),
                )
                setReplyTarget(null)
                toast.error('Demo — replies need the reviews service (BACKEND-GAPS.md G7)')
              }}
            >
              Post reply
            </Button>
          ) : undefined
        }
      >
        {replyTarget ? (
          <>
            <DrawerSection label={`From ${replyTarget.customer}`}>
              <div className="text-sm text-text2">“{replyTarget.comment}”</div>
            </DrawerSection>
            <DrawerSection label="Your reply">
              <Textarea
                rows={4}
                value={reply}
                onChange={(e) => setReply(e.target.value)}
                placeholder="Customers read replies before they order — be you, be brief."
              />
              <div className="text-xs text-text3">
                {replyTarget.reply
                  ? 'You already replied — posting again replaces the reply.'
                  : 'The customer is notified when you post.'}
              </div>
            </DrawerSection>
          </>
        ) : null}
      </Drawer>
    </div>
  )
}
