import { ChatProvider } from './ChatProvider';
import { ChatLayout } from './ChatLayout';

export default function ChatPage() {
  return (
    <ChatProvider>
      <ChatLayout />
    </ChatProvider>
  );
}
