import { useChatContext } from './ChatProvider';
import { MessageArea } from './components/MessageArea';
import { MessageInput } from './components/MessageInput';
import { ChatSidebar } from './sidebar/ChatSidebar';

export function ChatLayout() {
  const {
    messages,
    thinking,
    isStreaming,
    status,
    pendingInteraction,
    sendMessage,
    cancel,
    respondToInteraction,
    startNewSession,
    activeTools,
    toolHistory,
  } = useChatContext();

  return (
    <>
      <div className="flex-1 flex flex-col">
        <MessageArea
          messages={messages}
          thinking={thinking}
          isStreaming={isStreaming}
          status={status}
          pendingInteraction={pendingInteraction}
          onSendSuggestion={sendMessage}
          onRespondToInteraction={respondToInteraction}
        />
        <MessageInput
          onSend={sendMessage}
          onCancel={cancel}
          onNewSession={startNewSession}
          isStreaming={isStreaming}
        />
      </div>
      <ChatSidebar
        status={status}
        isStreaming={isStreaming}
        thinking={thinking}
        activeTools={activeTools}
        toolHistory={toolHistory}
      />
    </>
  );
}
