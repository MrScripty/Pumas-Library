/**
 * What an update-stream subscription delivers: a decoded notification, or a
 * resync request when a payload or the stream itself could not be delivered.
 * A resync means "reload from the RPC snapshot"; it carries no cursor.
 */
export type UpdateStreamDelivery<T> =
  | { readonly kind: 'notification'; readonly notification: T }
  | { readonly kind: 'resync'; readonly reason: string };
