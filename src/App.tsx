import { SearchBox } from "./components/SearchBox";
import "./index.css";

function App() {
  return (
    <div className="min-h-screen w-full flex items-start justify-center pt-20">
      <div className="w-full max-w-2xl rounded-lg shadow-2xl overflow-hidden">
        <SearchBox />
      </div>
    </div>
  );
}

export default App;
