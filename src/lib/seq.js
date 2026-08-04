// Stale-response guard (§7.5 of SPEC.md).
//
// Each `render_preview` request carries a monotonically increasing `seq`. When
// a response arrives, the frontend drops it if it is older than the latest
// request already sent — rapid typing can otherwise interleave responses out
// of order.

/** True when a response with `responseSeq` is older than the latest request
 *  (`latestSentSeq`) and must be discarded. */
export function isStaleResponse(responseSeq, latestSentSeq) {
  return responseSeq < latestSentSeq;
}
