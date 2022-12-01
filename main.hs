
import Debug.Trace

fold :: b -> (b -> a -> b) -> [a] -> b
fold init fn [head] = fn init head
fold init fn (head:tail) = fn (fold init fn tail) head

transition :: Int -> Char -> Int
transition 0 't' = 1
transition 1 'e' = 2
transition 2 's' = 3
transition 3 't' = 4
transition v c = -1

-- returns the final state
automaton :: (Int -> Char -> Int) -> Int -> String -> Int
automaton t_fn init word = fold init t_fn (reverse word)

run_automaton :: (Int -> Char -> Int) -> Int -> [Int] -> String -> Bool
run_automaton t_fn init final word = (automaton t_fn init word) `elem` final

-- run:run_automaton transition 0 [4] "test"

