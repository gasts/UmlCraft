#include <stdio.h>

int total;

int simpleIf(int a)
{
    if (a > 0) {
        a = 1;
    }
    return a;
}

int elseIfChain(int a)
{
    if (a < 0) {
        return -1;
    } else if (a == 0) {
        return 0;
    } else {
        return 1;
    }
}

int counting(int n)
{
    int sum = 0;
    int i;
    for (i = 0; i < n; i++) {
        sum += i;
    }
    for (i = n; i > 0; i--) {
        sum -= i;
    }
    return sum;
}

int whileLoop(int n)
{
    while (n > 10) {
        n = n / 2;
    }
    do {
        n--;
    } while (n > 0);
    return n;
}

int classic(int day)
{
    int r = 0;
    switch (day) {
    case 1:
        r = 10;
        break;
    case 2:
    case 3:
        r = 20;
        break;
    default:
        r = 0;
    }
    return r;
}

int fallthrough(int a)
{
    int r = 0;
    switch (a) {
    case 1:
        r += 1;
    case 2:
        r += 2;
        break;
    default:
        r = -1;
        break;
    }
    return r;
}

void calls(int n)
{
    helper(n, 1);
    total = helper(n, 2) + 1;
    printf("%d\n", total);
}

int helper(int a, int b)
{
    return a + b;
}
