/**
 * GraphQL subscription for real-time notifications.
 */

export const NOTIFICATION_SUBSCRIPTION = `
  subscription OnNotification {
    notification {
      id
      type
      title
      body
      read
      timestamp
      href
    }
  }
`;

export const MARK_ALL_READ = `
  mutation MarkAllNotificationsRead {
    markAllNotificationsRead {
      success
    }
  }
`;

export const GET_NOTIFICATIONS = `
  query GetNotifications($limit: Int, $offset: Int) {
    notifications(limit: $limit, offset: $offset) {
      id
      type
      title
      body
      read
      timestamp
      href
    }
  }
`;
