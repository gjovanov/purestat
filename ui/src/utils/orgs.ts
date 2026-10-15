/** The fields the org picker needs: an id to select and a name to show. */
export interface OrgOption {
  id: string
  name: string
}

/**
 * The org picker's items: the user's orgs, plus the current org when the list
 * does not hold it yet.
 *
 * A page opened directly (a bookmark, a reload, the landing after sign-in)
 * fetches only its own org, so the list can still be empty while the page
 * already knows its current org. A select whose value is not among its items
 * shows the raw value, which here is the org's id. Keeping the current org
 * in the items shows its name from the first render.
 */
export function orgOptions<T extends OrgOption>(orgs: readonly T[], current: T | null): T[] {
  if (!current || orgs.some((o) => o.id === current.id)) return [...orgs]
  return [current, ...orgs]
}
