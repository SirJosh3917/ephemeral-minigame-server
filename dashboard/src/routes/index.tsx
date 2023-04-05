import { createSignal, onCleanup, Show } from "solid-js";
import { isServer } from "solid-js/web";
import Computer from "~/components/Computer";
import { StatusResp } from "./api/status";

export default function Home() {
  const [computers, setComputers] = createSignal<StatusResp>([]);
  const [hasFetched, setHasFetched] = createSignal(false);

  if (!isServer) {
    const interval = setInterval(async () => {
      const resp = await fetch("/api/status");
      const status: StatusResp = await resp.json();
      setHasFetched(true);
      setComputers(status);
    }, 1000);
    onCleanup(() => clearInterval(interval));
  }

  return (
    <main class="m-8 flex flex-wrap text-gray-200">
      <Show when={hasFetched()} fallback={<div>Loading...</div>}>
        <Show
          when={computers().length > 0}
          fallback={<div>No computers :&lt;</div>}
        >
          {computers().map(({ name: computerName, status }) => (
            <Computer name={computerName} state={status} />
          ))}
        </Show>
      </Show>
    </main>
  );
}
