export default function Loading() {
  return (
    <div aria-busy="true" aria-label="Loading" className="flex flex-col gap-10">
      <div className="skeleton h-16 w-2/3 rounded-sm bg-line" />
      <div className="grid grid-cols-2 gap-6 md:grid-cols-4">
        {[0, 1, 2, 3].map((i) => (
          <div key={i} className="skeleton h-20 rounded-sm bg-line" />
        ))}
      </div>
      <div className="skeleton h-56 rounded-sm bg-line" />
      <div className="flex flex-col gap-2">
        {[0, 1, 2, 3, 4, 5].map((i) => (
          <div key={i} className="skeleton h-10 rounded-sm bg-line" />
        ))}
      </div>
    </div>
  );
}
