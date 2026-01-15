import { useMemo } from 'react';
import type { Entry } from '../types';
import type { GroupByDimension } from './useEntriesFilters';

export interface EntryGroup {
  /** Unique identifier for the group */
  key: string;
  /** Display label for the group header */
  label: string;
  /** Number of entries in this group */
  count: number;
  /** Entries belonging to this group */
  entries: Entry[];
}

/**
 * Hook to compute entry groups based on a selected dimension.
 * Supports grouping by source_domain, entry_type, source_type, or tags.
 */
export function useEntryGroups(
  entries: Entry[],
  groupBy: GroupByDimension
): EntryGroup[] {
  return useMemo(() => {
    // No grouping - return all entries as single group
    if (groupBy === 'none') {
      return [{
        key: '__all__',
        label: 'All Entries',
        count: entries.length,
        entries,
      }];
    }

    const groupsMap = new Map<string, Entry[]>();

    entries.forEach((entry) => {
      if (groupBy === 'tag') {
        // Tags are arrays - entry can appear in multiple groups
        const tags = entry.tags?.length ? entry.tags : ['Untagged'];
        tags.forEach((tag) => {
          const existing = groupsMap.get(tag) || [];
          existing.push(entry);
          groupsMap.set(tag, existing);
        });
      } else {
        // Single-value properties
        let value: string;
        switch (groupBy) {
          case 'source_domain':
            value = entry.source_domain || 'Unknown Source';
            break;
          case 'entry_type':
            value = entry.entry_type || 'Unknown Type';
            break;
          case 'source_type':
            value = entry.source_type || 'Unknown Format';
            break;
          default:
            value = 'Unknown';
        }
        const existing = groupsMap.get(value) || [];
        existing.push(entry);
        groupsMap.set(value, existing);
      }
    });

    // Convert to array and sort by count (descending)
    return Array.from(groupsMap.entries())
      .map(([key, groupEntries]) => ({
        key,
        label: formatGroupLabel(key, groupBy),
        count: groupEntries.length,
        entries: groupEntries,
      }))
      .sort((a, b) => b.count - a.count);
  }, [entries, groupBy]);
}

/**
 * Format group labels for display.
 * Handles special cases and applies appropriate formatting.
 */
function formatGroupLabel(key: string, groupBy: GroupByDimension): string {
  // Handle special placeholder values
  if (key.startsWith('Unknown') || key === 'Untagged') {
    return key;
  }

  switch (groupBy) {
    case 'source_domain':
      // Keep domains as-is (they're already formatted)
      return key;
    case 'entry_type':
    case 'source_type':
      // Capitalize and format
      return key.charAt(0).toUpperCase() + key.slice(1).toLowerCase();
    case 'tag':
      // Tags are user-defined, keep original casing
      return key;
    default:
      return key;
  }
}

/**
 * Get group statistics for display.
 */
export function useGroupStats(groups: EntryGroup[]) {
  return useMemo(() => {
    const totalGroups = groups.length;
    const totalEntries = groups.reduce((sum, g) => sum + g.count, 0);
    const avgEntriesPerGroup = totalGroups > 0 ? Math.round(totalEntries / totalGroups) : 0;
    const largestGroup = groups.length > 0 ? groups[0] : null;

    return {
      totalGroups,
      totalEntries,
      avgEntriesPerGroup,
      largestGroup,
    };
  }, [groups]);
}
