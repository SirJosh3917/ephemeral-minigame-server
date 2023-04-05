import { FaSolidComputer } from "solid-icons/fa";

interface ComputerProps {
  name: string;
  state: "starting" | "online";
}

export default function Computer(props: ComputerProps) {
  return (
    <div class="flex flex-col m-4">
      <div class="flex mx-auto">
        <FaSolidComputer
          size={128}
          color={props.state === "starting" ? "gray" : "white"}
        />
      </div>
      <div
        class={
          "flex flex-col " +
          (props.state === "starting" ? "text-gray-400" : "text-white")
        }
      >
        <div class="mx-auto">{props.name}</div>
        <div class="mx-auto">
          {props.state === "starting" ? "Starting..." : "Online!"}
        </div>
      </div>
    </div>
  );
}
