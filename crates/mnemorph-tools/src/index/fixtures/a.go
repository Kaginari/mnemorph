package main

import (
	"fmt"
	"os"
)

const Limit = 10

// Stack holds items.
type Stack struct {
	items []int
}

type Shape interface {
	Area() float64
}

// Push adds one.
func (s *Stack) Push(n int) {
	s.items = append(s.items, n)
}

func main() {
	fmt.Println(os.Args)
}
