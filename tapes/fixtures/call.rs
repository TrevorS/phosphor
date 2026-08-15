// The call `<C-s>` gets asked inside. This file declares nothing: the float is
// the server's words, not this file's.
//
// The two shouting lines are the assertion. They are the rows the float
// covers, and `signature-help.tape`'s header says why that is the only shape
// the assertion can take. The tail is load-bearing: the tape counts back to
// the first of them (`G k k k O`).

pub fn main() {
    UNDER_THE_FLOAT_IS_THIS_LINE;
    AND_THIS_ONE_IS_UNDER_IT;
}
